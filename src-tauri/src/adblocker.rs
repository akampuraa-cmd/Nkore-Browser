//! Ad-blocker and tracker-blocking module for Nkore Browser.
//!
//! Provides two layers of protection:
//!   1. JavaScript initialization scripts injected into every content page that
//!      override `fetch` and `XMLHttpRequest` to block requests to known ad/
//!      tracker domains.
//!   2. CSS snippets injected to hide common ad placeholder elements.

/// A curated list of ad-network and tracker domains.
/// This is a compact but representative subset of EasyList / EasyPrivacy.
pub const BLOCKED_DOMAINS: &[&str] = &[
    // Google Ads / Analytics
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "googletagmanager.com",
    "googletagservices.com",
    "google-analytics.com",
    "analytics.google.com",
    // Facebook Pixel / Tracking
    "connect.facebook.net",
    "facebook.com/tr",
    "fbcdn.net",
    // Amazon Ads
    "amazon-adsystem.com",
    "assoc-amazon.com",
    // Twitter/X Ads
    "ads-twitter.com",
    "t.co",
    "static.ads-twitter.com",
    // Microsoft Ads
    "bat.bing.com",
    "msads.net",
    "adnxs.com",
    // Outbrain / Taboola
    "outbrain.com",
    "taboola.com",
    "revcontent.com",
    // Criteo
    "criteo.com",
    "criteo.net",
    // Yahoo / Oath Ads
    "ads.yahoo.com",
    "adtech.de",
    "yastatic.net",
    "gemini.yahoo.com",
    // Tracking pixels & analytics
    "scorecardresearch.com",
    "omtrdc.net",
    "demdex.net",
    "advertising.com",
    "openx.net",
    "openx.com",
    "pubmatic.com",
    "rubiconproject.com",
    "moatads.com",
    "rlcdn.com",
    "quantserve.com",
    "bluekai.com",
    "krxd.net",
    "lotame.com",
    "nexac.com",
    "mathtag.com",
    "mediamath.com",
    "turn.com",
    "casalemedia.com",
    "indexww.com",
    "sovrn.com",
    "lijit.com",
    "contextweb.com",
    "pulsepoint.com",
    "districtm.ca",
    "triplelift.com",
    "smartadserver.com",
    "smaato.net",
    "appnexus.com",
    "bidswitch.net",
    "bidswitch.com",
    "spotxchange.com",
    "sharethrough.com",
    "undertone.com",
    "yieldmo.com",
    "synacor.com",
    "advertising.com",
    "adform.net",
    "adbutter.net",
    "adalyser.com",
    "adnxs.com",
    "ads.linkedin.com",
    "ads.pinterest.com",
    "ads.snapchat.com",
    "ads.tiktok.com",
    // Heatmaps / session recorders
    "hotjar.com",
    "mouseflow.com",
    "luckyorange.com",
    "fullstory.com",
    "logrocket.com",
    "heap.io",
    // Crash reporting (can phone home user data)
    "bugsnag.com",
    "sentry.io",
    "rollbar.com",
    // A/B testing / optimization beacons
    "optimizely.com",
    "vwo.com",
    "abtasty.com",
    "conductrics.com",
];

/// Returns the JavaScript content-script that should be injected into every
/// content page (via `WebviewBuilder::initialization_script()`).
///
/// The script overrides `fetch`, `XMLHttpRequest`, and `WebSocket` to block
/// requests matching any entry in `BLOCKED_DOMAINS`, and also injects CSS to
/// collapse/hide common ad-related DOM elements.
pub fn build_blocker_script() -> String {
    let domains_json = serde_json::to_string(BLOCKED_DOMAINS).unwrap_or_default();

    format!(
        r#"
(function() {{
    'use strict';

    // ── Blocked domain list ────────────────────────────────────────────────
    const BLOCKED = {domains};

    function isBlocked(urlStr) {{
        if (!urlStr) return false;
        try {{
            const host = new URL(urlStr).hostname.toLowerCase();
            return BLOCKED.some(d => host === d || host.endsWith('.' + d));
        }} catch (_) {{
            return false;
        }}
    }}

    // ── Override fetch ─────────────────────────────────────────────────────
    const _fetch = window.fetch;
    window.fetch = function(input, init) {{
        const url = (input instanceof Request) ? input.url : String(input);
        if (isBlocked(url)) {{
            console.debug('[Nkore] Blocked fetch:', url);
            return Promise.reject(new TypeError('Nkore AdBlocker blocked: ' + url));
        }}
        return _fetch.apply(this, arguments);
    }};

    // ── Override XMLHttpRequest ────────────────────────────────────────────
    const _open = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url) {{
        if (isBlocked(url)) {{
            console.debug('[Nkore] Blocked XHR:', url);
            // Mark this instance as blocked so send() is a no-op
            this.__nkoreBlocked = true;
            return;
        }}
        this.__nkoreBlocked = false;
        return _open.apply(this, arguments);
    }};
    const _send = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.send = function() {{
        if (this.__nkoreBlocked) return;
        return _send.apply(this, arguments);
    }};

    // ── Override WebSocket ─────────────────────────────────────────────────
    const _WS = window.WebSocket;
    window.WebSocket = function(url, protocols) {{
        if (isBlocked(url)) {{
            console.debug('[Nkore] Blocked WebSocket:', url);
            throw new DOMException('Nkore AdBlocker blocked: ' + url, 'SecurityError');
        }}
        return protocols ? new _WS(url, protocols) : new _WS(url);
    }};
    Object.setPrototypeOf(window.WebSocket, _WS);

    // ── CSS ad-element hiding ──────────────────────────────────────────────
    const AD_SELECTORS = [
        '[id*="google_ads"]', '[id*="googlead"]', '[class*="GoogleAds"]',
        '[id*="ad-container"]', '[class*="ad-container"]',
        '[id*="adsbygoogle"]', '.adsbygoogle',
        'iframe[src*="doubleclick.net"]',
        'iframe[src*="googlesyndication"]',
        '[id*="taboola"]', '[class*="taboola"]',
        '[id*="outbrain"]', '[class*="outbrain"]',
        '[class*="sponsored-post"]', '[class*="promoted-post"]',
        '[aria-label="Sponsored"]', '[aria-label="Advertisement"]',
    ].join(',');

    function injectAdCSS() {{
        const style = document.createElement('style');
        style.id = 'nkore-ad-blocker';
        style.textContent = AD_SELECTORS + ' {{ display: none !important; }}';
        if (document.head) {{
            document.head.appendChild(style);
        }} else {{
            document.addEventListener('DOMContentLoaded', () => {{
                document.head && document.head.appendChild(style);
            }});
        }}
    }}
    injectAdCSS();

    // ── Media Sniffer registry ─────────────────────────────────────────────
    // Continuously collect media URLs as the page loads so the sniffer panel
    // can read them on demand.
    window.__nkoreMedia = [];

    function collectMediaFromElement(el) {{
        const src = el.src || el.currentSrc || el.getAttribute('src') || el.getAttribute('data-src') || el.getAttribute('data-lazy-src');
        if (!src || src.startsWith('data:') || window.__nkoreMedia.find(m => m.url === src)) return;
        window.__nkoreMedia.push({{
            url: src,
            type: el.tagName.toLowerCase(),
            title: el.alt || el.title || el.getAttribute('aria-label') || '',
            width: el.naturalWidth || el.videoWidth || 0,
            height: el.naturalHeight || el.videoHeight || 0,
        }});
    }}

    function scanAllMedia() {{
        document.querySelectorAll('video, audio, img, source').forEach(collectMediaFromElement);
        // Also check link[rel=preload] for media
        document.querySelectorAll('link[rel="preload"][as="video"],link[rel="preload"][as="audio"],link[rel="preload"][as="image"]').forEach(el => {{
            const src = el.href;
            if (src && !window.__nkoreMedia.find(m => m.url === src)) {{
                window.__nkoreMedia.push({{ url: src, type: el.getAttribute('as'), title: '', width: 0, height: 0 }});
            }}
        }});
    }}

    // Observe DOM mutations to catch dynamically loaded media
    const observer = new MutationObserver((mutations) => {{
        for (const m of mutations) {{
            m.addedNodes.forEach(node => {{
                if (node.nodeType === 1) {{
                    if (['VIDEO','AUDIO','IMG','SOURCE'].includes(node.tagName)) {{
                        collectMediaFromElement(node);
                    }}
                    node.querySelectorAll && node.querySelectorAll('video,audio,img,source').forEach(collectMediaFromElement);
                }}
            }});
        }}
    }});
    observer.observe(document.documentElement, {{ childList: true, subtree: true }});

    document.addEventListener('DOMContentLoaded', scanAllMedia);
    window.addEventListener('load', scanAllMedia);

    // Expose scan function for on-demand use
    window.__nkoreScanMedia = function() {{
        scanAllMedia();
        return JSON.stringify(window.__nkoreMedia);
    }};

}})();
"#,
        domains = domains_json
    )
}

/// Returns a minimal CSS snippet that hides the most common ad placeholders.
/// This is injected separately for pages that load before the JS runs.
pub fn build_ad_css() -> &'static str {
    r#"
[id*="google_ads"], [id*="adsbygoogle"], .adsbygoogle,
iframe[src*="doubleclick.net"], iframe[src*="googlesyndication"],
[id*="taboola"], [class*="taboola"],
[id*="outbrain"], [class*="outbrain"],
[class*="sponsored-post"], [class*="promoted-post"] {
    display: none !important;
}
"#
}
