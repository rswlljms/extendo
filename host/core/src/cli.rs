// CLI parsing for the capture core. Kept dependency-free and pure so the
// security rule below is unit-testable.
//
// The Node host spawns this process and passes the already-entitlement-capped
// width/height/fps (AGENTS.md section 10.1 keeps all tier logic in
// host/src/license.js - never duplicate paywall checks here).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub port: u16,
    pub bind: String,
    pub token: String,
    /// 0 = primary monitor, N = 1-based monitor index (multi-phone routing).
    pub monitor: usize,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u8,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            port: 9578,
            // Loopback by default: ADB-reverse works, and a missing --bind can
            // never silently expose the desktop to the LAN.
            bind: "127.0.0.1".to_string(),
            token: String::new(),
            monitor: 0,
            width: 1280,
            height: 720,
            fps: 30,
            quality: 70,
        }
    }
}

impl Args {
    /// Parse `--key value` pairs. Unknown keys are rejected rather than ignored
    /// so a typo in a flag can't silently fall back to an insecure default.
    ///
    /// # Errors
    /// Returns a message describing the offending flag.
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Self, String> {
        let mut out = Self::default();
        let v: Vec<String> = args.into_iter().collect();
        let mut i = 0;
        while i < v.len() {
            let key = v[i].as_str();
            let val = v.get(i + 1);
            let need = |v: Option<&String>| -> Result<String, String> {
                v.cloned().ok_or_else(|| format!("{key} needs a value"))
            };
            match key {
                "--port" => out.port = need(val)?.parse().map_err(|_| "bad --port".to_string())?,
                "--bind" => out.bind = need(val)?,
                "--token" => out.token = need(val)?,
                "--monitor" => {
                    out.monitor = need(val)?.parse().map_err(|_| "bad --monitor".to_string())?;
                }
                "--width" => out.width = need(val)?.parse().map_err(|_| "bad --width".to_string())?,
                "--height" => {
                    out.height = need(val)?.parse().map_err(|_| "bad --height".to_string())?;
                }
                "--fps" => out.fps = need(val)?.parse().map_err(|_| "bad --fps".to_string())?,
                "--quality" => {
                    out.quality = need(val)?.parse().map_err(|_| "bad --quality".to_string())?;
                }
                other => return Err(format!("unknown flag {other}")),
            }
            i += 2;
        }
        out.validate()?;
        Ok(out)
    }

    /// AGENTS.md section 7.6: never listen on a non-loopback address without a
    /// pairing token, and keep the stream inside the v1 1080p60 SDR ceiling.
    ///
    /// # Errors
    /// Returns a message describing the violated constraint.
    pub fn validate(&self) -> Result<(), String> {
        if self.token.is_empty() && !is_loopback(&self.bind) {
            return Err(format!(
                "refusing to bind {} without --token (AGENTS.md 7.6: no open bind without auth)",
                self.bind
            ));
        }
        if self.fps == 0 || self.fps > 60 {
            return Err("--fps must be 1..=60 (v1 caps at 60)".to_string());
        }
        if self.width == 0 || self.height == 0 || self.width > 1920 || self.height > 1080 {
            return Err("--width/--height must fit within 1920x1080 (v1 SDR cap)".to_string());
        }
        if self.quality < 1 || self.quality > 100 {
            return Err("--quality must be 1..=100".to_string());
        }
        Ok(())
    }

    /// Frame interval derived from the fps cap.
    #[must_use]
    pub fn frame_interval(&self) -> std::time::Duration {
        std::time::Duration::from_micros(1_000_000 / u64::from(self.fps.max(1)))
    }
}

/// Loopback check that also covers `localhost` and IPv6 `::1`.
#[must_use]
pub fn is_loopback(addr: &str) -> bool {
    addr == "127.0.0.1" || addr == "localhost" || addr == "::1" || addr.starts_with("127.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Result<Args, String> {
        Args::parse(s.iter().map(|x| (*x).to_string()))
    }

    #[test]
    fn defaults_are_loopback_and_720p30() {
        let a = Args::default();
        assert_eq!(a.bind, "127.0.0.1");
        assert_eq!((a.width, a.height, a.fps), (1280, 720, 30));
        assert!(a.validate().is_ok());
    }

    #[test]
    fn parses_flags() {
        let a = args(&[
            "--port", "9600", "--bind", "0.0.0.0", "--token", "abc", "--monitor", "2", "--width",
            "1920", "--height", "1080", "--fps", "60", "--quality", "80",
        ])
        .expect("parse");
        assert_eq!(a.port, 9600);
        assert_eq!(a.bind, "0.0.0.0");
        assert_eq!(a.token, "abc");
        assert_eq!(a.monitor, 2);
        assert_eq!((a.width, a.height, a.fps, a.quality), (1920, 1080, 60, 80));
    }

    #[test]
    fn rejects_public_bind_without_token() {
        // The core security invariant: no unauthenticated LAN exposure.
        let err = args(&["--bind", "0.0.0.0"]).unwrap_err();
        assert!(err.contains("--token"), "unexpected error: {err}");
    }

    #[test]
    fn allows_public_bind_with_token() {
        assert!(args(&["--bind", "0.0.0.0", "--token", "s3cret"]).is_ok());
    }

    #[test]
    fn allows_loopback_without_token_for_adb_reverse() {
        // ADB-reverse target (AGENTS.md section 2) must work with no token.
        assert!(args(&["--bind", "127.0.0.1"]).is_ok());
        assert!(args(&["--bind", "localhost"]).is_ok());
    }

    #[test]
    fn rejects_out_of_range_video_settings() {
        assert!(args(&["--fps", "0"]).is_err());
        assert!(args(&["--fps", "120"]).is_err(), "v1 caps at 60fps");
        assert!(args(&["--width", "3840", "--height", "2160"]).is_err(), "no 4K in v1");
        assert!(args(&["--quality", "0"]).is_err());
    }

    #[test]
    fn rejects_unknown_and_valueless_flags() {
        assert!(args(&["--nope", "1"]).is_err());
        assert!(args(&["--port"]).is_err());
    }

    #[test]
    fn frame_interval_matches_fps() {
        let a = args(&["--fps", "30"]).expect("parse");
        assert_eq!(a.frame_interval(), std::time::Duration::from_micros(33_333));
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("127.0.0.53"));
        assert!(is_loopback("::1"));
        assert!(!is_loopback("0.0.0.0"));
        assert!(!is_loopback("192.168.1.10"));
    }
}
