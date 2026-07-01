import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";

interface Props {
  value: number;
  onChange: (hours: number) => void;
  disabled?: boolean;
}

export function HoursInput({ value, onChange, disabled }: Props) {
  const [text, setText] = useState(value > 0 ? String(value) : "");

  useEffect(() => {
    setText(value > 0 ? String(value) : "");
  }, [value]);

  const commit = (raw: string) => {
    setText(raw);
    const n = Number(raw);
    if (Number.isFinite(n) && n >= 0 && n <= 24) {
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
