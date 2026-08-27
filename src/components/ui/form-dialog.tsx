import * as React from "react";

import { DialogContent } from "@/components/ui/dialog";

type FormDialogContextValue = {
  // Call from controls that emit no DOM `input` event — Radix Select and
  // Checkbox render their popup in a portal, so nothing bubbles to the content.
  markDirty: () => void;
};

const FormDialogContext = React.createContext<FormDialogContextValue | null>(null);

const NO_DIALOG: FormDialogContextValue = { markDirty: () => {} };

/** Returns a no-op outside a FormDialogContent, so the same form component can
 *  also render inline on a page. */
export function useFormDialog(): FormDialogContextValue {
  const ctx = React.useContext(FormDialogContext);
  return ctx ?? NO_DIALOG;
}

/**
 * A DialogContent for forms: once the user has entered anything, a stray click
 * on the overlay or a stray Escape stops throwing that input away. Deliberate
 * exits — the X, the form's cancel button — are left alone.
 *
 * Dirtiness resets by itself: Radix unmounts the content on close, taking this
 * component's state with it.
 */
export function FormDialogContent({
  children,
  dirty: dirtyProp = false,
  ...props
}: React.ComponentPropsWithoutRef<typeof DialogContent> & {
  // For a component that renders FormDialogContent itself and therefore sits
  // above the provider, useFormDialog() is out of reach — pass the signal in.
  dirty?: boolean;
}) {
  const [dirtySelf, setDirtySelf] = React.useState(false);
  const dirty = dirtyProp || dirtySelf;
  const markDirty = React.useCallback(() => setDirtySelf(true), []);
  const value = React.useMemo(() => ({ markDirty }), [markDirty]);

  return (
    <FormDialogContext.Provider value={value}>
      <DialogContent
        {...props}
        onInput={markDirty}
        onPointerDownOutside={(e) => { if (dirty) e.preventDefault(); }}
        onEscapeKeyDown={(e) => { if (dirty) e.preventDefault(); }}
      >
        {children}
      </DialogContent>
    </FormDialogContext.Provider>
  );
}
