import { useEffect, useRef, useState } from "react";
import { Input } from "@/components/ui/input";

interface Props {
  value: number;
  onChange: (hours: number) => void;
  disabled?: boolean;
}

export function HoursInput({ value, onChange, disabled }: Props) {
  const [text, setText] = useState(String(value));
  // Skip the next parent-driven sync when the value change was caused by our own typing.
  // Otherwise typing "2." → onChange(2) → parent → useEffect setText(String(2))="2" would
  // clobber the trailing dot and prevent decimal entry.
  const isTypingRef = useRef(false);

  useEffect(() => {
    if (isTypingRef.current) {
      isTypingRef.current = false;
      return;
    }
    setText(String(value));
  }, [value]);

  const commit = (raw: string) => {
    setText(raw);
    const n = Number(raw);
    if (Number.isFinite(n) && n >= 0 && n <= 24) {
      isTypingRef.current = true;
      onChange(n);
    }
  };

  return (
    <div className="flex items-center gap-2">
      <Input
        inputMode="decimal"
        type="number"
        min="0"
        max="24"
        step="0.25"
        value={text}
        disabled={disabled}
        placeholder="8"
        onChange={(e) => commit(e.target.value)}
      />
      <span className="text-sm text-muted-foreground">小时</span>
    </div>
  );
}
