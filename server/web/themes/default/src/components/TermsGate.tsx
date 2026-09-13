import { createSignal, type JSX } from "solid-js";
import { acceptTerms, hasAcceptedTerms, PRIVACY_URL, TERMS_URL } from "../lib/compliance.ts";
import { Dialog } from "./Dialog.tsx";

export function TermsGate(): JSX.Element {
  const [accepted, setAccepted] = createSignal(hasAcceptedTerms());

  function accept(): void {
    acceptTerms();
    setAccepted(true);
  }

  return (
    <Dialog open={!accepted()} title="Terms of use" onClose={() => undefined}>
      <p>
        Use only content that you have the right to use. Treat other people with respect. You can
        report playlists and accounts after you sign in.
      </p>
      <p class="hint">
        Read the <a href={TERMS_URL} target="_blank" rel="noreferrer">terms</a> and the{" "}
        <a href={PRIVACY_URL} target="_blank" rel="noreferrer">privacy policy</a>.
      </p>
      <div class="dialog-actions">
        <button type="button" class="btn btn-primary" onClick={accept}>
          I accept
        </button>
      </div>
    </Dialog>
  );
}
