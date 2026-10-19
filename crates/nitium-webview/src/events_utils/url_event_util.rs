// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use servo::ServoUrl;

/// Parses an absolute URL. Interpreting what the user typed (search terms, bare
/// domains, …) is the application's job (`nitium_core::input::resolve`); anything
/// that still does not parse here opens as a blank page.
pub fn parse_servo_url(url: &str) -> ServoUrl {
    ServoUrl::parse(url).unwrap_or_else(|_| {
        ServoUrl::parse("about:blank").expect("about:blank is a valid URL")
    })
}
