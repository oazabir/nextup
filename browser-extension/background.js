chrome.tabs.onActivated.addListener(async ({ windowId }) => {
  try {
    const browserWindow = await chrome.windows.get(windowId);
    if (!browserWindow.focused) return;

    await fetch('http://127.0.0.1:43817/tab-switch', {
      method: 'POST',
      headers: { 'X-Next-Up-Event': 'tab-switch' },
      credentials: 'omit'
    });
  } catch {
    // Next Up can be closed while the browser is running.
  }
});
