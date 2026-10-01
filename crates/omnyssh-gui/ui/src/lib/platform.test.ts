import { describe, expect, it } from 'vitest';
import { dropPoint } from './platform';

// Where an OS drop lands, from the position each webview reports (tauri-runtime-wry
// wraps it unscaled): it decides which pane and folder row take the drop.
describe('dropPoint', () => {
  const reported = { x: 1800, y: 600 };

  it('scales WebView2 physical pixels down on Windows', () => {
    expect(dropPoint(reported, true, 1.5)).toEqual({ x: 1200, y: 400 });
    expect(dropPoint(reported, true, 1)).toEqual(reported);
  });

  it('keeps WKWebView points as they are on a Retina Mac', () => {
    expect(dropPoint(reported, false, 2)).toEqual(reported);
  });

  it('keeps WebKitGTK logical pixels as they are under GDK_SCALE=2', () => {
    expect(dropPoint(reported, false, 2)).toEqual(reported);
  });
});
