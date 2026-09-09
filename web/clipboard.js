"use strict";

async function copyTaskText(name, description, status) {
  const details = description?.trim();
  const value = details ? `${name} - ${details}` : name;
  status.hidden = false;
  status.textContent = "Copying…";
  if (!navigator.clipboard?.writeText) {
    status.textContent =
      "Clipboard unavailable. Open this page over HTTPS or localhost to copy.";
    return;
  }
  try {
    await navigator.clipboard.writeText(value);
    status.textContent = "Copied to clipboard";
  } catch {
    status.textContent =
      "Could not copy. Allow clipboard access in your browser and try again.";
  }
}
