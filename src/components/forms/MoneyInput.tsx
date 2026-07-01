import { useEffect, useRef, useState } from "react";
import { Input } from "@/components/ui/input";
import { fromCents, toCents } from "@/lib/money";

interface Props {
  value: number; // cents
  onChange: (cents: number) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function MoneyInput({ value, onChange, disabled, placeholder }: Props) {
  const [text, setText] = useState(value > 0 ? fromCents(value) : "");
  // Skip the next parent-driven sync when the value change was caused by our own typing.
  // Without this, typing "1." would round-trip through onChange → parent → useEffect and
  // overwrite the input with "1.00", making decimals unreachable.
  const isTypingRef = useRef(false);

  useEffect(() => {
    if (isTypingRef.current) {
      isTypingRef.current = false;
      return;
    }
    setText(value > 0 ? fromCents(value) : "");
  }, [value]);

  const commit = (raw: string) => {
    setText(raw);
    try {
      isTypingRef.current = true;
      onChange(toCents(raw));
    } catch {
      // keep text; do not propagate invalid value
      isTypingRef.current = false;
    }
  };

  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-muted-foreground">¥</span>
      <Input
        inputMode="decimal"
        value={text}
        disabled={disabled}
        placeholder={placeholder ?? "0.00"}
        onChange={(e) => commit(e.target.value)}
      />
    </div>
  );
}
