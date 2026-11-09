import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Loader2, LockKeyhole, Mail, ShieldCheck, User } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Alert } from "@/components/ui/alert";
import { authClient } from "@/lib/auth-client";

const schema = z.object({
  name: z.string().optional(),
  email: z.string().email("Ingrese un correo válido."),
  password: z.string().min(8, "La contraseña debe tener al menos 8 caracteres."),
});

type FormValues = z.infer<typeof schema>;

export default function LoginPage() {
  const [mode, setMode] = useState<"signin" | "register">("signin");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", email: "", password: "" },
  });

  const onSubmit = async (values: FormValues) => {
    setError("");
    setBusy(true);
    try {
      const result =
        mode === "signin"
          ? await authClient.signIn.email({
              email: values.email,
              password: values.password,
            })
          : await authClient.signUp.email({
              name: values.name?.trim() || values.email.split("@")[0],
              email: values.email,
              password: values.password,
            });

      if (result.error) {
        setError(result.error.message ?? "No se pudo completar la operación.");
        return;
      }
      window.location.assign("/");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Error inesperado.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="relative flex min-h-screen items-center justify-center overflow-hidden bg-background px-4 py-10">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_20%_20%,oklch(0.66_0.14_250/0.16),transparent_45%),radial-gradient(circle_at_80%_80%,oklch(0.68_0.15_155/0.1),transparent_45%)]" />
      <div className="relative grid w-full max-w-4xl gap-8 lg:grid-cols-[1.1fr_1fr] lg:items-center">
        <div className="hidden lg:block">
          <div className="mb-5 flex items-center gap-3">
            <img
              src="/icono.png"
              alt="Parte Diario"
              className="h-12 w-12 rounded-xl object-cover ring-1 ring-border"
            />
            <div>
              <h1 className="text-2xl font-semibold tracking-tight">Parte Diario</h1>
              <p className="text-sm text-muted-foreground">
                Anti-Ruidos y Contaminación Sónica · P.N.
              </p>
            </div>
          </div>
          <p className="max-w-md text-sm leading-relaxed text-muted-foreground">
            Convierta el export de WhatsApp en el Parte Diario oficial en Word: agrupe novedades,
            formalice narrativas y genere el <code>.docx</code> con el membrete institucional en un
            solo flujo.
          </p>
          <ul className="mt-6 space-y-3 text-sm text-muted-foreground">
            {[
              "Detección de duplicados por SHA-256 y por contenido",
              "Pre-selección automática de lo posterior al último reporte",
              "Formalización con IA o limpieza sin conexión",
              "Validación OOXML antes de descargar el documento",
            ].map((item) => (
              <li key={item} className="flex items-start gap-2">
                <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-[var(--success)]" />
                {item}
              </li>
            ))}
          </ul>
        </div>

        <Card className="border-border/70 bg-card/90 shadow-2xl backdrop-blur">
          <CardContent className="p-6">
            <div className="mb-5 flex items-center gap-3 lg:hidden">
              <img
                src="/icono.png"
                alt="Parte Diario"
                className="h-10 w-10 rounded-lg object-cover ring-1 ring-border"
              />
              <div>
                <div className="text-base font-semibold">Parte Diario P.N.</div>
                <div className="text-[11px] text-muted-foreground">
                  Anti-Ruidos y Contaminación Sónica
                </div>
              </div>
            </div>

            <Tabs
              value={mode}
              onValueChange={(value) => {
                setMode(value as "signin" | "register");
                setError("");
              }}
            >
              <TabsList className="grid w-full grid-cols-2">
                <TabsTrigger value="signin">Entrar</TabsTrigger>
                <TabsTrigger value="register">Crear cuenta</TabsTrigger>
              </TabsList>
            </Tabs>

            <form className="mt-5 space-y-4" onSubmit={form.handleSubmit(onSubmit)}>
              {mode === "register" ? (
                <div className="space-y-1.5">
                  <Label htmlFor="name">Nombre</Label>
                  <div className="relative">
                    <User className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      id="name"
                      className="pl-9"
                      placeholder="Digitador N3"
                      autoComplete="name"
                      {...form.register("name")}
                    />
                  </div>
                  {form.formState.errors.name ? (
                    <p className="text-xs text-destructive">{form.formState.errors.name.message}</p>
                  ) : null}
                </div>
              ) : null}

              <div className="space-y-1.5">
                <Label htmlFor="email">Correo</Label>
                <div className="relative">
                  <Mail className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="email"
                    type="email"
                    className="pl-9"
                    placeholder="agente@policia.gob.do"
                    autoComplete="email"
                    {...form.register("email")}
                  />
                </div>
                {form.formState.errors.email ? (
                  <p className="text-xs text-destructive">{form.formState.errors.email.message}</p>
                ) : null}
              </div>

              <div className="space-y-1.5">
                <Label htmlFor="password">Contraseña</Label>
                <div className="relative">
                  <LockKeyhole className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="password"
                    type="password"
                    className="pl-9"
                    placeholder="Mínimo 8 caracteres"
                    autoComplete={mode === "signin" ? "current-password" : "new-password"}
                    {...form.register("password")}
                  />
                </div>
                {form.formState.errors.password ? (
                  <p className="text-xs text-destructive">
                    {form.formState.errors.password.message}
                  </p>
                ) : null}
              </div>

              {error ? <Alert variant="destructive">{error}</Alert> : null}

              <Button className="w-full" size="lg" disabled={busy}>
                {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                {mode === "signin" ? "Entrar al sistema" : "Crear cuenta"}
              </Button>
            </form>

            <p className="mt-4 text-center text-[11px] leading-relaxed text-muted-foreground">
              Instalación de uso interno. La API key del modelo se guarda cifrada en PostgreSQL y
              nunca se envía al navegador.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
