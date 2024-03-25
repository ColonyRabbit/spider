use crate::utils::log;
use crate::{configuration::Configuration, tokio_stream::StreamExt};
use chromiumoxide::Page;
use chromiumoxide::{handler::HandlerConfig, Browser, BrowserConfig};
use tokio::task::{self, JoinHandle};

/// get chrome configuration
#[cfg(not(feature = "chrome_headed"))]
pub fn get_browser_config(
    proxies: &Option<Box<Vec<string_concat::String>>>,
    intercept: bool,
    cache_enabled: bool,
    viewport: impl Into<Option<chromiumoxide::handler::viewport::Viewport>>,
    request_timeout: &Option<Box<core::time::Duration>>,
) -> Option<BrowserConfig> {
    let builder = BrowserConfig::builder()
        .disable_default_args()
        .request_timeout(match request_timeout.as_ref() {
            Some(timeout) => **timeout,
            _ => Default::default(),
        });

    let builder = if cache_enabled {
        builder.enable_cache()
    } else {
        builder.disable_cache()
    };

    // request interception is required for all browser.new_page() creations. We also have to use "about:blank" as the base page to setup the listeners and navigate afterwards or the request will hang.
    let builder = if cfg!(feature = "chrome_intercept") && intercept {
        builder.enable_request_intercept()
    } else {
        builder
    };

    let builder = match proxies {
        Some(proxies) => {
            let mut chrome_args = Vec::from(CHROME_ARGS.map(|e| e.replace("://", "=").to_string()));

            chrome_args.push(string_concat!(
                r#"--proxy-server=""#,
                proxies.join(";"),
                r#"""#
            ));

            builder.args(chrome_args)
        }
        _ => builder.args(CHROME_ARGS),
    };
    let builder = if std::env::var("CHROME_BIN").is_ok() {
        match std::env::var("CHROME_BIN") {
            Ok(v) => builder.chrome_executable(v),
            _ => builder,
        }
    } else {
        builder
    };

    match builder.viewport(viewport).build() {
        Ok(b) => Some(b),
        Err(error) => {
            log("", error);
            None
        }
    }
}

/// get chrome configuration headful
#[cfg(feature = "chrome_headed")]
pub fn get_browser_config(
    proxies: &Option<Box<Vec<string_concat::String>>>,
    intercept: bool,
    cache_enabled: bool,
    viewport: impl Into<Option<chromiumoxide::handler::viewport::Viewport>>,
    request_timeout: &Option<Box<core::time::Duration>>,
) -> Option<BrowserConfig> {
    let builder = BrowserConfig::builder()
        .disable_default_args()
        .no_sandbox()
        .request_timeout(match request_timeout.as_ref() {
            Some(timeout) => **timeout,
            _ => Default::default(),
        })
        .with_head();

    let builder = if cache_enabled {
        builder.enable_cache()
    } else {
        builder.disable_cache()
    };

    let builder = if cfg!(feature = "chrome_intercept") && intercept {
        builder.enable_request_intercept()
    } else {
        builder
    };

    let mut chrome_args = Vec::from(CHROME_ARGS.map(|e| {
        if e == "--headless" {
            "".to_string()
        } else {
            e.replace("://", "=").to_string()
        }
    }));

    let builder = match proxies {
        Some(proxies) => {
            chrome_args.push(string_concat!(
                r#"--proxy-server=""#,
                proxies.join(";"),
                r#"""#
            ));

            builder.args(chrome_args)
        }
        _ => builder.args(chrome_args),
    };
    let builder = if std::env::var("CHROME_BIN").is_ok() {
        match std::env::var("CHROME_BIN") {
            Ok(v) => builder.chrome_executable(v),
            _ => builder,
        }
    } else {
        builder
    };
    match builder.viewport(viewport).build() {
        Ok(b) => Some(b),
        Err(error) => {
            log("", error);
            None
        }
    }
}

/// launch a chromium browser and wait until the instance is up.
pub async fn launch_browser(
    config: &Configuration,
) -> Option<(Browser, tokio::task::JoinHandle<()>)> {
    let proxies = &config.proxies;

    let browser_configuration = match std::env::var("CHROME_URL") {
        Ok(v) => match Browser::connect_with_config(
            &v,
            HandlerConfig {
                request_timeout: match config.request_timeout.as_ref() {
                    Some(timeout) => **timeout,
                    _ => Default::default(),
                },
                request_intercept: cfg!(feature = "chrome_intercept") && config.chrome_intercept,
                cache_enabled: config.cache,
                viewport: config.viewport.clone(),
                ..HandlerConfig::default()
            },
        )
        .await
        {
            Ok(browser) => Some(browser),
            Err(err) => {
                log::error!("{:?}", err);
                None
            }
        },
        _ => match get_browser_config(
            &proxies,
            config.chrome_intercept,
            config.cache,
            config.viewport.clone(),
            &config.request_timeout,
        ) {
            Some(browser_config) => match Browser::launch(browser_config).await {
                Ok(browser) => Some(browser),
                _ => None,
            },
            _ => None,
        },
    };

    match browser_configuration {
        Some(c) => {
            let (browser, mut handler) = c;

            // spawn a new task that continuously polls the handler
            let handle = task::spawn(async move {
                while let Some(h) = handler.next().await {
                    if h.is_err() {
                        break;
                    }
                }
            });

            Some((browser, handle))
        }
        _ => None,
    }
}

/// configure the browser
pub async fn configure_browser(new_page: Page, configuration: &Configuration) -> Page {
    let new_page = match configuration.timezone_id.as_deref() {
        Some(timezone_id) => {
            match new_page
                .emulate_timezone(
                    chromiumoxide::cdp::browser_protocol::emulation::SetTimezoneOverrideParams::new(
                        timezone_id,
                    ),
                )
                .await
            {
                Ok(np) => np.to_owned(),
                _ => new_page,
            }
        }
        _ => new_page,
    };
    let new_page = match configuration.locale.as_deref() {
        Some(locale) => {
            match new_page
                .emulate_locale(
                    chromiumoxide::cdp::browser_protocol::emulation::SetLocaleOverrideParams {
                        locale: Some(locale.into()),
                    },
                )
                .await
            {
                Ok(np) => np.to_owned(),
                _ => new_page,
            }
        }
        _ => new_page,
    };

    new_page
}

/// close the browser and open handles
pub async fn close_browser(browser_handle: JoinHandle<()>) {
    if !browser_handle.is_finished() {
        browser_handle.abort();
    }
}

/// static chrome arguments to start
#[cfg(all(feature = "chrome_cpu", feature = "cloudflare_bypass"))]
pub static CHROME_ARGS: [&'static str; 27] = [
    if cfg!(feature = "chrome_headless_new") {
        "--headless=new"
    } else {
        "--headless"
    },
    "--disable-extensions",
    "--disable-component-extensions-with-background-pages",
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-client-side-phishing-detection",
    "--disable-sync",
    "--metrics-recording-only",
    "--disable-default-apps",
    "--mute-audio",
    "--no-default-browser-check",
    "--no-first-run",
    "--disable-gpu",
    "--disable-gpu-sandbox",
    "--disable-setuid-sandbox",
    "--disable-dev-shm-usage",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
    "--disable-background-timer-throttling",
    "--disable-ipc-flooding-protection",
    "--password-store=basic",
    "--use-mock-keychain",
    "--force-fieldtrials=*BackgroundTracing/default/",
    "--disable-hang-monitor",
    "--disable-prompt-on-repost",
    "--disable-domain-reliability",
    "--disable-features=InterestFeedContentSuggestions,PrivacySandboxSettings4,AutofillServerCommunication,CalculateNativeWinOcclusion,OptimizationHints,AudioServiceOutOfProcess,IsolateOrigins,site-per-process,ImprovedCookieControls,LazyFrameLoading,GlobalMediaControls,DestroyProfileOnBrowserClose,MediaRouter,DialMediaRouteProvider,AcceptCHFrame,AutoExpandDetailsElement,CertificateTransparencyComponentUpdater,AvoidUnnecessaryBeforeUnloadCheckSync,Translate"
];

/// static chrome arguments to start
#[cfg(all(not(feature = "chrome_cpu"), feature = "cloudflare_bypass"))]
pub static CHROME_ARGS: [&'static str; 24] = [
    if cfg!(feature = "chrome_headless_new") {
        "--headless=new"
    } else {
        "--headless"
    },
    "--disable-extensions",
    "--disable-component-extensions-with-background-pages",
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-client-side-phishing-detection",
    "--disable-sync",
    "--disable-dev-shm-usage",
    "--metrics-recording-only",
    "--disable-default-apps",
    "--mute-audio",
    "--no-default-browser-check",
    "--no-first-run",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
    "--disable-background-timer-throttling",
    "--disable-ipc-flooding-protection",
    "--password-store=basic",
    "--use-mock-keychain",
    "--force-fieldtrials=*BackgroundTracing/default/",
    "--disable-hang-monitor",
    "--disable-prompt-on-repost",
    "--disable-domain-reliability",
    "--disable-features=InterestFeedContentSuggestions,PrivacySandboxSettings4,AutofillServerCommunication,CalculateNativeWinOcclusion,OptimizationHints,AudioServiceOutOfProcess,IsolateOrigins,site-per-process,ImprovedCookieControls,LazyFrameLoading,GlobalMediaControls,DestroyProfileOnBrowserClose,MediaRouter,DialMediaRouteProvider,AcceptCHFrame,AutoExpandDetailsElement,CertificateTransparencyComponentUpdater,AvoidUnnecessaryBeforeUnloadCheckSync,Translate"
];

// One of the configs below is detected by CF bots. We need to take a look at the optimal args 03/25/24.

#[cfg(all(not(feature = "chrome_cpu"), not(feature = "cloudflare_bypass")))]
/// static chrome arguments to start application ref [https://github.com/a11ywatch/chrome/blob/main/src/main.rs#L13]
static CHROME_ARGS: [&'static str; 60] = [
    if cfg!(feature = "chrome_headless_new") { "--headless=new" } else { "--headless" },
    "--no-sandbox",
    "--no-first-run",
    "--hide-scrollbars",
    // "--allow-pre-commit-input",
    // "--user-data-dir=~/.config/google-chrome",
    "--allow-running-insecure-content",
    "--autoplay-policy=user-gesture-required",
    "--ignore-certificate-errors",
    "--no-default-browser-check",
    "--no-zygote",
    "--disable-setuid-sandbox",
    "--disable-dev-shm-usage", // required or else docker containers may crash not enough memory
    "--disable-threaded-scrolling",
    "--disable-demo-mode",
    "--disable-dinosaur-easter-egg",
    "--disable-fetching-hints-at-navigation-start",
    "--disable-site-isolation-trials",
    "--disable-web-security",
    "--disable-threaded-animation",
    "--disable-sync",
    "--disable-print-preview",
    "--disable-partial-raster",
    "--disable-in-process-stack-traces",
    "--disable-v8-idle-tasks",
    "--disable-low-res-tiling",
    "--disable-speech-api",
    "--disable-smooth-scrolling",
    "--disable-default-apps",
    "--disable-prompt-on-repost",
    "--disable-domain-reliability",
    "--disable-component-update",
    "--disable-background-timer-throttling",
    "--disable-breakpad",
    "--disable-software-rasterizer",
    "--disable-extensions",
    "--disable-popup-blocking",
    "--disable-hang-monitor",
    "--disable-image-animation-resync",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-ipc-flooding-protection",
    "--disable-background-networking",
    "--disable-renderer-backgrounding",
    "--disable-field-trial-config",
    "--disable-back-forward-cache",
    "--disable-backgrounding-occluded-windows",
    "--force-fieldtrials=*BackgroundTracing/default/",
    // "--enable-automation",
    "--log-level=3",
    "--enable-logging=stderr",
    "--enable-features=SharedArrayBuffer,NetworkService,NetworkServiceInProcess",
    "--metrics-recording-only",
    "--use-mock-keychain",
    "--force-color-profile=srgb",
    "--mute-audio",
    "--no-service-autorun",
    "--password-store=basic",
    "--export-tagged-pdf",
    "--no-pings",
    "--use-gl=swiftshader",
    "--window-size=1920,1080",
    "--disable-features=InterestFeedContentSuggestions,PrivacySandboxSettings4,AutofillServerCommunication,CalculateNativeWinOcclusion,OptimizationHints,AudioServiceOutOfProcess,IsolateOrigins,site-per-process,ImprovedCookieControls,LazyFrameLoading,GlobalMediaControls,DestroyProfileOnBrowserClose,MediaRouter,DialMediaRouteProvider,AcceptCHFrame,AutoExpandDetailsElement,CertificateTransparencyComponentUpdater,AvoidUnnecessaryBeforeUnloadCheckSync,Translate"
];

#[cfg(all(feature = "chrome_cpu", not(feature = "cloudflare_bypass")))]
/// static chrome arguments to start application ref [https://github.com/a11ywatch/chrome/blob/main/src/main.rs#L13]
static CHROME_ARGS: [&'static str; 63] = [
    if cfg!(feature = "chrome_headless_new") { "--headless=new" } else { "--headless" },
    "--no-sandbox",
    "--no-first-run",
    "--hide-scrollbars",
    // "--allow-pre-commit-input",
    // "--user-data-dir=~/.config/google-chrome",
    "--allow-running-insecure-content",
    "--autoplay-policy=user-gesture-required",
    "--ignore-certificate-errors",
    "--no-default-browser-check",
    "--no-zygote",
    "--in-process-gpu",
    "--disable-gpu",
    "--disable-gpu-sandbox",
    "--disable-setuid-sandbox",
    "--disable-dev-shm-usage", // required or else docker containers may crash not enough memory
    "--disable-threaded-scrolling",
    "--disable-demo-mode",
    "--disable-dinosaur-easter-egg",
    "--disable-fetching-hints-at-navigation-start",
    "--disable-site-isolation-trials",
    "--disable-web-security",
    "--disable-threaded-animation",
    "--disable-sync",
    "--disable-print-preview",
    "--disable-partial-raster",
    "--disable-in-process-stack-traces",
    "--disable-v8-idle-tasks",
    "--disable-low-res-tiling",
    "--disable-speech-api",
    "--disable-smooth-scrolling",
    "--disable-default-apps",
    "--disable-prompt-on-repost",
    "--disable-domain-reliability",
    "--disable-component-update",
    "--disable-background-timer-throttling",
    "--disable-breakpad",
    "--disable-software-rasterizer",
    "--disable-extensions",
    "--disable-popup-blocking",
    "--disable-hang-monitor",
    "--disable-image-animation-resync",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-ipc-flooding-protection",
    "--disable-background-networking",
    "--disable-renderer-backgrounding",
    "--disable-field-trial-config",
    "--disable-back-forward-cache",
    "--disable-backgrounding-occluded-windows",
    "--force-fieldtrials=*BackgroundTracing/default/",
    // "--enable-automation",
    "--log-level=3",
    "--enable-logging=stderr",
    "--enable-features=SharedArrayBuffer,NetworkService,NetworkServiceInProcess",
    "--metrics-recording-only",
    "--use-mock-keychain",
    "--force-color-profile=srgb",
    "--mute-audio",
    "--no-service-autorun",
    "--password-store=basic",
    "--export-tagged-pdf",
    "--no-pings",
    "--use-gl=swiftshader",
    "--window-size=1920,1080",
    "--disable-features=InterestFeedContentSuggestions,PrivacySandboxSettings4,AutofillServerCommunication,CalculateNativeWinOcclusion,OptimizationHints,AudioServiceOutOfProcess,IsolateOrigins,site-per-process,ImprovedCookieControls,LazyFrameLoading,GlobalMediaControls,DestroyProfileOnBrowserClose,MediaRouter,DialMediaRouteProvider,AcceptCHFrame,AutoExpandDetailsElement,CertificateTransparencyComponentUpdater,AvoidUnnecessaryBeforeUnloadCheckSync,Translate"
];
