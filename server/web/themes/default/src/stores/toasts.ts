import { showNotice } from "@catalystcommunity/menus";
export function useToast() {
  return { show(message: string, action?: { label: string; run: () => void }) {
    showNotice({ text: message, timeout: action ? 8000 : 6000,
      action: action ? { ...action, mnemonic: false } : undefined });
  } };
}
