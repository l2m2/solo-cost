import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useAuthStore } from "@/stores/auth";

export default function SetupPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const setup = useAuthStore((s) => s.setup);
  const [pwd, setPwd] = useState("");
  const [confirm, setConfirm] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    if (pwd.length < 8) return toast.error(t("setup.passwordMin"));
    if (pwd !== confirm) return toast.error(t("setup.passwordMismatch"));
    setSubmitting(true);
    try {
      await setup(pwd);
      navigate("/dashboard", { replace: true });
    } catch (e: any) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>{t("setup.title")}</CardTitle>
          <CardDescription>{t("setup.warning")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="space-y-1">
            <Label>{t("setup.password")}</Label>
            <Input type="password" value={pwd} onChange={(e) => setPwd(e.target.value)} autoFocus />
          </div>
          <div className="space-y-1">
            <Label>{t("setup.confirm")}</Label>
            <Input type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
          </div>
          <Button className="w-full" onClick={submit} disabled={submitting}>
            {t("setup.submit")}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
