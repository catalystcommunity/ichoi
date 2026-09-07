import { createContext, For, onCleanup, Show, useContext, type JSX, type ParentProps } from "solid-js";
import { createStore } from "solid-js/store";

interface ToastAction {
  label: string;
  run: () => void;
}

interface Toast {
  id: number;
  message: string;
  action?: ToastAction;
}

interface ToastContextValue {
  show: (message: string, action?: ToastAction) => void;
}

const ToastContext = createContext<ToastContextValue>();

export function ToastProvider(props: ParentProps): JSX.Element {
  const [toasts, setToasts] = createStore<Toast[]>([]);
  const timers = new Map<number, number>();
  let nextId = 1;

  function dismiss(id: number): void {
    const timer = timers.get(id);
    if (timer) window.clearTimeout(timer);
    timers.delete(id);
    setToasts((list) => list.filter((t) => t.id !== id));
  }

  function show(message: string, action?: ToastAction): void {
    const id = nextId++;
    setToasts((list) => [...list, { id, message, action }]);
    timers.set(id, window.setTimeout(() => dismiss(id), action ? 8000 : 4200));
  }

  onCleanup(() => {
    for (const timer of timers.values()) window.clearTimeout(timer);
    timers.clear();
  });

  return (
    <ToastContext.Provider value={{ show }}>
      {props.children}
      <div class="toast-stack" role="status" aria-live="polite">
        <For each={toasts}>
          {(toast) => (
            <div class="toast">
              <span>{toast.message}</span>
              <Show when={toast.action}>
                {(action) => (
                  <button
                    type="button"
                    class="toast-action"
                    onClick={() => {
                      dismiss(toast.id);
                      action().run();
                    }}
                  >
                    {action().label}
                  </button>
                )}
              </Show>
              <button
                type="button"
                class="toast-dismiss"
                aria-label="Dismiss"
                onClick={() => dismiss(toast.id)}
              >
                ×
              </button>
            </div>
          )}
        </For>
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within <ToastProvider>");
  return ctx;
}
