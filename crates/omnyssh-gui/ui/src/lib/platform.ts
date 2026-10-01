// The platform the webview runs on, from its user agent: WKWebView says Macintosh,
// WebView2 Windows NT, WebKitGTK X11; Linux. app.html makes the same macOS test for the
// title-bar inset, and the e2e suite emulates a platform by its user agent alone.
export const isMac = /Mac/.test(navigator.userAgent);
export const isWindows = /Windows/.test(navigator.userAgent);

/** An OS drag-and-drop position in CSS pixels. Tauri passes on what the webview
 *  reports: WebView2 counts physical pixels, WKWebView and WebKitGTK logical ones. */
export function dropPoint(
  position: { x: number; y: number },
  windows = isWindows,
  ratio = globalThis.devicePixelRatio || 1
): { x: number; y: number } {
  return windows ? { x: position.x / ratio, y: position.y / ratio } : { ...position };
}
