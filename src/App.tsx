import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";

export default function App() {
  const [pong, setPong] = useState<string>("...");
  useEffect(() => {
    invoke<string>("ping").then(setPong).catch((e) => setPong(`error: ${e}`));
  }, []);
  return (
    <div className="min-h-screen flex flex-col items-center justify-center gap-4">
      <Button>solo-cost 启动成功</Button>
      <div className="text-sm text-muted-foreground">ipc: {pong}</div>
    </div>
  );
}
