import { useEffect, useState } from "react";
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

  useEffect(() => {
    // sync when parent resets value (e.g., dialog close)
    setText(value > 0 ? fromCents(value) : "");
  }, [value]);

  const commit = (raw: string) => {
    setText(raw);
    try {
      onChange(toCents(raw));
    } catch {
      // keep text; do not propagate invalid value
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
