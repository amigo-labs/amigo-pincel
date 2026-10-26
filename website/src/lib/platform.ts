// The visitor's desktop system, for the download button. Runs in the
// browser only; nothing is sent or stored.

/** A system Pincel ships a desktop build for. */
export type Platform = 'windows' | 'macos' | 'linux';

/**
 * The desktop build for this browser's system, or `null` where there is
 * none: phones and tablets (iPadOS presents itself as a Mac with a touch
 * screen), ChromeOS and ARM Linux. Windows on ARM runs the x64 build.
 */
export function detectPlatform(nav: Navigator): Platform | null {
  const ua = nav.userAgent;
  const uad = (nav as Navigator & { userAgentData?: { mobile?: boolean; platform?: string } })
    .userAgentData;
  if (uad?.mobile || /Android|iPhone|iPad|iPod/i.test(ua)) return null;
  if (/Mac/i.test(ua) && nav.maxTouchPoints > 1) return null;
  const os = uad?.platform || ua;
  if (/Win/i.test(os)) return 'windows';
  if (/Mac/i.test(os)) return 'macos';
  if (/Linux|X11/i.test(os) && !/CrOS|aarch64|arm64|armv/i.test(ua)) return 'linux';
  return null;
}
