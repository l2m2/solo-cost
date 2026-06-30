import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useAuthStore } from "@/stores/auth";

export default function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const unlock = useAuthStore((s) => s.unlock);
  const [pwd, setPwd] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    setSubmitting(true);
    try {
      await unlock(pwd);
      navigate("/dashboard", { replace: true });
    } catch (e: any) {
      const msg = String(e);
      if (msg.includes("wrong master password")) toast.error(t("login.wrongPassword"));
      else toast.error(t("common.error", { msg }));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>{t("login.title")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="space-y-1">
            <Label>{t("login.password")}</Label>
            <Input type="password" value={pwd} onChange={(e) => setPwd(e.target.value)} autoFocus
              onKeyDown={(e) => e.key === "Enter" && submit()} />
          </div>
          <Button className="w-full" onClick={submit} disabled={submitting}>
            {t("login.submit")}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
