import { useEffect, useRef, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { FileUp, KeyRound, Loader2, Save, Star, Trash2 } from "lucide-react";
import type { AppSettings, TemplateSummary } from "@parte/shared";
import { api, errorText } from "@/lib/api";
import { PROVIDER_OPTIONS } from "@/lib/providers";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/misc";

const schema = z.object({
  provider: z.enum(["openrouter", "anthropic", "freellmapi"]),
  model: z.string().min(1, "Indique el modelo."),
  apiKey: z.string().optional(),
  anonymize: z.boolean(),
  defaultTemplateId: z.string(),
});

type FormValues = z.infer<typeof schema>;

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);

  const settingsQuery = useQuery({
    queryKey: ["settings"],
    queryFn: () => api<AppSettings>("/api/settings"),
  });
  const templatesQuery = useQuery({
    queryKey: ["templates"],
    queryFn: () => api<TemplateSummary[]>("/api/templates"),
  });

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      provider: "openrouter",
      model: "google/gemma-4-31b-it:free",
      apiKey: "",
      anonymize: true,
      defaultTemplateId: "builtin",
    },
  });

  useEffect(() => {
    if (!settingsQuery.data) return;
    form.reset({
      provider: settingsQuery.data.provider,
      model: settingsQuery.data.model,
      apiKey: "",
      anonymize: settingsQuery.data.anonymize,
      defaultTemplateId: settingsQuery.data.defaultTemplateId ?? "builtin",
    });
  }, [settingsQuery.data, form]);

  const saveMutation = useMutation({
    mutationFn: (values: FormValues) =>
      api<AppSettings>("/api/settings", {
        method: "PUT",
        body: JSON.stringify({
          provider: values.provider,
          model: values.model,
          anonymize: values.anonymize,
          defaultTemplateId:
            values.defaultTemplateId === "builtin" ? null : values.defaultTemplateId,
          ...(values.apiKey?.trim() ? { apiKey: values.apiKey.trim() } : {}),
        }),
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(["settings"], data);
      queryClient.invalidateQueries({ queryKey: ["templates"] });
      form.setValue("apiKey", "");
      toast.success("Ajustes guardados.");
    },
    onError: (error) => toast.error(errorText(error)),
  });

  const clearKeyMutation = useMutation({
    mutationFn: () =>
      api<AppSettings>("/api/settings", {
        method: "PUT",
        body: JSON.stringify({ apiKey: "" }),
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(["settings"], data);
      toast.success("API key eliminada.");
    },
    onError: (error) => toast.error(errorText(error)),
  });

  const uploadMutation = useMutation({
    mutationFn: async (file: File) => {
      const data = new FormData();
      data.append("file", file);
      return api<TemplateSummary>("/api/templates", { method: "POST", body: data });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["templates"] });
      toast.success("Plantilla cargada.");
    },
    onError: (error) => toast.error(errorText(error)),
  });

  const setDefaultMutation = useMutation({
    mutationFn: (id: string) => api(`/api/templates/${id}/default`, { method: "PATCH" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["templates"] });
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      toast.success("Plantilla predeterminada actualizada.");
    },
    onError: (error) => toast.error(errorText(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api(`/api/templates/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["templates"] });
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      toast.success("Plantilla eliminada.");
    },
    onError: (error) => toast.error(errorText(error)),
  });

  const provider = form.watch("provider");
  const providerInfo = PROVIDER_OPTIONS.find((p) => p.value === provider);
  const settings = settingsQuery.data;

  return (
    <div className="mx-auto grid max-w-5xl gap-5 lg:grid-cols-[1.1fr_0.9fr]">
      <Card>
        <CardHeader>
          <CardTitle>Redacción con IA</CardTitle>
          <CardDescription>
            Proveedor, modelo y privacidad de los datos enviados al modelo.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-5"
            onSubmit={form.handleSubmit((values) => saveMutation.mutate(values))}
          >
            <div className="space-y-1.5">
              <Label>Proveedor</Label>
              <Select
                value={form.watch("provider")}
                onChange={(event) => {
                  const value = event.target.value as FormValues["provider"];
                  form.setValue("provider", value);
                  const info = PROVIDER_OPTIONS.find((p) => p.value === value);
                  if (info) form.setValue("model", info.defaultModel);
                }}
              >
                {PROVIDER_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </Select>
              {providerInfo ? (
                <p className="text-xs text-muted-foreground">{providerInfo.hint}</p>
              ) : null}
            </div>

            <div className="space-y-1.5">
              <Label htmlFor="model">Modelo</Label>
              <Input id="model" {...form.register("model")} />
              {form.formState.errors.model ? (
                <p className="text-xs text-destructive">{form.formState.errors.model.message}</p>
              ) : null}
            </div>

            <div className="space-y-1.5">
              <Label htmlFor="apiKey">API key</Label>
              <div className="relative">
                <KeyRound className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  id="apiKey"
                  type="password"
                  className="pl-9"
                  placeholder={
                    settings?.apiKeySet
                      ? `Guardada (${settings.apiKeyPreview}) — escriba para reemplazar`
                      : "Pegue su API key"
                  }
                  {...form.register("apiKey")}
                />
              </div>
              {settings?.apiKeySet ? (
                <div className="flex items-center gap-2 pt-1">
                  <Badge variant="success">Clave configurada</Badge>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 text-destructive"
                    onClick={() => clearKeyMutation.mutate()}
                    disabled={clearKeyMutation.isPending}
                  >
                    Quitar clave
                  </Button>
                </div>
              ) : (
                <p className="text-xs text-muted-foreground">
                  Sin clave, use la limpieza sin conexión en el flujo principal.
                </p>
              )}
            </div>

            <div className="flex items-center justify-between rounded-lg border border-border/70 px-4 py-3">
              <div>
                <div className="text-sm font-medium">Anonimizar antes de enviar</div>
                <p className="text-xs text-muted-foreground">
                  Sustituye cédulas, teléfonos, correos y nombres; se restauran en la respuesta.
                </p>
              </div>
              <Switch
                checked={form.watch("anonymize")}
                onCheckedChange={(checked) => form.setValue("anonymize", checked)}
              />
            </div>

            <div className="space-y-1.5">
              <Label>Plantilla predeterminada</Label>
              <Select {...form.register("defaultTemplateId")}>
                {(templatesQuery.data ?? []).map((template) => (
                  <option key={template.id} value={template.id}>
                    {template.name}
                    {template.isDefault ? " (actual)" : ""}
                  </option>
                ))}
              </Select>
            </div>

            <Button type="submit" disabled={saveMutation.isPending}>
              {saveMutation.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Save className="h-4 w-4" />
              )}
              Guardar ajustes
            </Button>
          </form>
        </CardContent>
      </Card>

      <div className="space-y-5">
        <Card>
          <CardHeader>
            <CardTitle>Plantillas .docx</CardTitle>
            <CardDescription>
              La plantilla aporta membrete, logo y formato de tabla. Sus tablas de novedades
              antiguas se eliminan al generar.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <input
              ref={fileRef}
              type="file"
              accept=".docx"
              className="hidden"
              onChange={async (event) => {
                const file = event.target.files?.[0];
                if (!file) return;
                setUploading(true);
                try {
                  await uploadMutation.mutateAsync(file);
                } finally {
                  setUploading(false);
                  if (fileRef.current) fileRef.current.value = "";
                }
              }}
            />
            <Button
              variant="outline"
              className="w-full"
              onClick={() => fileRef.current?.click()}
              disabled={uploading}
            >
              {uploading ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <FileUp className="h-4 w-4" />
              )}
              Cargar plantilla .docx
            </Button>

            <Separator />

            <div className="space-y-2">
              {(templatesQuery.data ?? []).map((template) => (
                <div
                  key={template.id}
                  className="flex items-center gap-2 rounded-lg border border-border/60 px-3 py-2"
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">{template.name}</div>
                    <div className="truncate text-[11px] text-muted-foreground">
                      {template.filename}
                    </div>
                  </div>
                  {template.isDefault ? (
                    <Badge variant="success">Predeterminada</Badge>
                  ) : (
                    <Button
                      variant="ghost"
                      size="icon"
                      title="Usar como predeterminada"
                      onClick={() => setDefaultMutation.mutate(template.id)}
                    >
                      <Star className="h-4 w-4" />
                    </Button>
                  )}
                  {template.id !== "builtin" ? (
                    <Button
                      variant="ghost"
                      size="icon"
                      title="Eliminar plantilla"
                      className="text-destructive"
                      onClick={() => {
                        if (window.confirm(`¿Eliminar "${template.name}"?`)) {
                          deleteMutation.mutate(template.id);
                        }
                      }}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  ) : null}
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        <Alert variant="info" title="Privacidad">
          La API key se cifra con AES-256-GCM en PostgreSQL. La anonimización es local: el navegador
          y los logs nunca reciben la clave ni los datos sensibles originales enviados al modelo.
        </Alert>
      </div>
    </div>
  );
}
