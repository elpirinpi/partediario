import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  AlertTriangle,
  ArrowRightLeft,
  Check,
  CheckCheck,
  CircleSlash,
  Download,
  FileArchive,
  FileText,
  Image as ImageIcon,
  Info,
  Loader2,
  Play,
  Save,
  Sparkles,
  Upload,
  Video,
} from "lucide-react";
import type {
  AppSettings,
  ChatSessionSummary,
  FormalizeResult,
  Novedad,
  ReportResult,
  TemplateSummary,
} from "@parte/shared";
import { ApiError, api, errorText } from "@/lib/api";
import { formatDate, truncate } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useWorkspace } from "@/store/workspace";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import { Checkbox } from "@/components/ui/checkbox";
import { Alert } from "@/components/ui/alert";
import { Separator } from "@/components/ui/misc";

export default function WorkspacePage() {
  const session = useWorkspace((s) => s.session);
  const setSession = useWorkspace((s) => s.setSession);
  const selectedId = useWorkspace((s) => s.selectedId);
  const setSelectedId = useWorkspace((s) => s.setSelectedId);
  const filter = useWorkspace((s) => s.filter);
  const setFilter = useWorkspace((s) => s.setFilter);
  const setNovedades = useWorkspace((s) => s.setNovedades);
  const updateNovedad = useWorkspace((s) => s.updateNovedad);

  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [templateId, setTemplateId] = useState("builtin");
  const [report, setReport] = useState<ReportResult | null>(null);
  const [replaceMessage, setReplaceMessage] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);

  const settingsQuery = useQuery({
    queryKey: ["settings"],
    queryFn: () => api<AppSettings>("/api/settings"),
  });
  const templatesQuery = useQuery({
    queryKey: ["templates"],
    queryFn: () => api<TemplateSummary[]>("/api/templates"),
  });

  useEffect(() => {
    if (!session) return;
    setDateFrom(session.dateFrom ?? "");
    setDateTo(session.dateTo ?? "");
    setReport(null);
    setReplaceMessage(null);
  }, [session?.id]);

  useEffect(() => {
    if (settingsQuery.data) {
      setTemplateId(settingsQuery.data.defaultTemplateId ?? "builtin");
    }
  }, [settingsQuery.data?.defaultTemplateId]);

  const uploadMutation = useMutation({
    mutationFn: async (file: File) => {
      const body = new FormData();
      body.append("file", file);
      return api<ChatSessionSummary>("/api/sessions", { method: "POST", body });
    },
    onSuccess: (data) => {
      setSession(data);
      toast.success(`ZIP procesado: ${data.novedades.length} novedades detectadas.`);
    },
    onError: (error) => toast.error(errorText(error)),
  });

  const saveMutation = useMutation({
    mutationFn: async () => {
      if (!session) throw new Error("No hay sesión activa.");
      return api<ChatSessionSummary>(`/api/sessions/${session.id}`, {
        method: "PATCH",
        body: JSON.stringify({
          novedades: session.novedades,
          dateFrom: dateFrom || null,
          dateTo: dateTo || null,
        }),
      });
    },
    onSuccess: () => toast.success("Borrador guardado."),
    onError: (error) => toast.error(errorText(error)),
  });

  const runFormalize = useCallback(
    async (ids: number[], useAi: boolean) => {
      if (!session) return;
      const targets = ids.filter((id) => {
        const n = session.novedades.find((item) => item.id === id);
        return n && n.bodyRaw.trim().length > 0;
      });
      if (targets.length === 0) {
        toast.info("No hay textos pendientes para formalizar.");
        return;
      }
      setProgress({ done: 0, total: targets.length });
      let errors = 0;
      for (const id of targets) {
        try {
          const results = await api<FormalizeResult[]>(`/api/sessions/${session.id}/formalize`, {
            method: "POST",
            body: JSON.stringify({ ids: [id], useAi }),
          });
          const result = results[0];
          if (result?.error) {
            errors += 1;
            toast.error(`Novedad #${id}: ${result.error}`);
          } else if (result) {
            updateNovedad(id, { bodyFormal: result.bodyFormal });
          }
        } catch (error) {
          errors += 1;
          toast.error(`Novedad #${id}: ${errorText(error)}`);
        }
        setProgress((prev) => (prev ? { done: prev.done + 1, total: prev.total } : null));
      }
      setProgress(null);
      if (errors === 0) {
        toast.success(
          useAi
            ? "Textos formalizados. Revíselos antes de generar."
            : "Limpieza sin conexión aplicada.",
        );
      } else {
        toast.warning(`Proceso terminado con ${errors} error(es). Revise las novedades.`);
      }
    },
    [session, updateNovedad],
  );

  const generate = useCallback(
    async (confirmReplace = false) => {
      if (!session) return;
      setGenerating(true);
      setReplaceMessage(null);
      try {
        const result = await api<ReportResult>(`/api/sessions/${session.id}/reports`, {
          method: "POST",
          body: JSON.stringify({
            dateFrom,
            dateTo,
            templateId,
            confirmReplace,
            novedades: session.novedades,
          }),
        });
        setReport(result);
        toast.success(
          `Parte Diario generado: ${result.novedadesIncluidas} novedades, ${result.imagesEmbedded} fotos.`,
        );
      } catch (error) {
        if (error instanceof ApiError && error.status === 409) {
          setReplaceMessage(error.message);
        } else {
          toast.error(errorText(error));
        }
      } finally {
        setGenerating(false);
      }
    },
    [session, dateFrom, dateTo, templateId],
  );

  const selected = session?.novedades.find((n) => n.id === selectedId) ?? null;
  const filtered = useMemo(() => {
    if (!session) return [];
    const q = filter.trim().toLowerCase();
    if (!q) return session.novedades;
    return session.novedades.filter((n) =>
      `${n.headers.join(" ")} ${n.bodyRaw} ${n.sender}`.toLowerCase().includes(q),
    );
  }, [session, filter]);

  const includedCount = session ? session.novedades.filter((n) => n.include).length : 0;
  const photoCount = session ? session.novedades.reduce((sum, n) => sum + n.photos.length, 0) : 0;

  return (
    <div className="grid gap-4 xl:grid-cols-[minmax(320px,390px)_1fr]">
      <div className="space-y-4">
        <UploadCard
          session={session}
          busy={uploadMutation.isPending}
          onFile={(file) => uploadMutation.mutate(file)}
          onReset={() => {
            setSession(null);
            setReport(null);
          }}
        />

        {session ? (
          <>
            <SessionSummary
              session={session}
              includedCount={includedCount}
              photoCount={photoCount}
            />
            <NovedadList
              novedades={filtered}
              total={session.novedades.length}
              selectedId={selectedId}
              filter={filter}
              onFilter={setFilter}
              onSelect={setSelectedId}
              onUpdate={updateNovedad}
              onSetAll={(include) =>
                setNovedades(session.novedades.map((n) => ({ ...n, include })))
              }
            />
          </>
        ) : (
          <HowItWorks />
        )}
      </div>

      <div className="space-y-4">
        {session && selected ? (
          <NovedadEditor
            key={selected.id}
            sessionId={session.id}
            novedad={selected}
            allNovedades={session.novedades}
            busy={progress !== null || uploadMutation.isPending}
            onUpdate={(patch) => updateNovedad(selected.id, patch)}
            onFormalize={() => runFormalize([selected.id], true)}
            onCleanup={() => runFormalize([selected.id], false)}
          />
        ) : session ? (
          <Card>
            <CardContent className="py-16 text-center text-sm text-muted-foreground">
              Seleccione una novedad para editarla.
            </CardContent>
          </Card>
        ) : (
          <Card className="min-h-[320px]">
            <CardContent className="flex h-full flex-col items-center justify-center gap-3 py-16 text-center">
              <div className="rounded-full bg-primary/10 p-4">
                <FileArchive className="h-8 w-8 text-primary" />
              </div>
              <div>
                <div className="text-base font-semibold">Cargue el .zip exportado de WhatsApp</div>
                <p className="mx-auto mt-1 max-w-md text-sm text-muted-foreground">
                  La app detecta el formato, agrupa las novedades, asocia las fotos y pre-selecciona
                  solo lo posterior a su último reporte.
                </p>
              </div>
            </CardContent>
          </Card>
        )}

        {session ? (
          <GenerateBar
            session={session}
            dateFrom={dateFrom}
            dateTo={dateTo}
            templateId={templateId}
            templates={templatesQuery.data ?? []}
            includedCount={includedCount}
            progress={progress}
            generating={generating}
            report={report}
            replaceMessage={replaceMessage}
            onDates={(from, to) => {
              setDateFrom(from);
              setDateTo(to);
            }}
            onTemplate={setTemplateId}
            onSave={() => saveMutation.mutate()}
            saving={saveMutation.isPending}
            onGenerate={() => generate(false)}
            onConfirmReplace={() => generate(true)}
            onFormalizeAll={() =>
              runFormalize(
                session.novedades.filter((n) => n.include).map((n) => n.id),
                true,
              )
            }
            onCleanupAll={() =>
              runFormalize(
                session.novedades.filter((n) => n.include).map((n) => n.id),
                false,
              )
            }
          />
        ) : null}
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Carga                                                               */
/* ------------------------------------------------------------------ */

function UploadCard({
  session,
  busy,
  onFile,
  onReset,
}: {
  session: ChatSessionSummary | null;
  busy: boolean;
  onFile: (file: File) => void;
  onReset: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2">
          <Upload className="h-4 w-4 text-primary" /> 1. Cargar .zip WhatsApp
        </CardTitle>
        <CardDescription>Exportar chat → Adjuntar archivos multimedia.</CardDescription>
      </CardHeader>
      <CardContent>
        <input
          ref={inputRef}
          type="file"
          accept=".zip"
          className="hidden"
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file) onFile(file);
            if (inputRef.current) inputRef.current.value = "";
          }}
        />
        <div
          onDragOver={(event) => {
            event.preventDefault();
            setDragOver(true);
          }}
          onDragLeave={() => setDragOver(false)}
          onDrop={(event) => {
            event.preventDefault();
            setDragOver(false);
            const file = event.dataTransfer.files?.[0];
            if (file && file.name.toLowerCase().endsWith(".zip")) onFile(file);
            else toast.error("Suelte un archivo .zip.");
          }}
          onClick={() => inputRef.current?.click()}
          className={cn(
            "flex cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed px-4 py-8 text-center transition-colors",
            dragOver
              ? "border-primary bg-primary/10"
              : "border-border/80 hover:border-primary/60 hover:bg-accent/40",
          )}
        >
          {busy ? (
            <Loader2 className="h-7 w-7 animate-spin text-primary" />
          ) : (
            <FileArchive className="h-7 w-7 text-muted-foreground" />
          )}
          <div className="text-sm font-medium">
            {busy ? "Procesando ZIP…" : "Arrastre el .zip o haga clic"}
          </div>
          <div className="text-[11px] text-muted-foreground">
            Solo se leen mensajes, fotos y videos. Los videos se omiten del .docx.
          </div>
        </div>
        {session ? (
          <div className="mt-3 flex items-center justify-between gap-2 rounded-lg bg-muted/60 px-3 py-2">
            <div className="min-w-0">
              <div className="truncate text-xs font-medium">{session.zipName}</div>
              <div className="text-[11px] text-muted-foreground">
                SHA-256 {session.zipSha256.slice(0, 12)}… · {session.txtName}
              </div>
            </div>
            <Button
              variant="ghost"
              size="sm"
              onClick={(event) => {
                event.stopPropagation();
                onReset();
              }}
            >
              Quitar
            </Button>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

function SessionSummary({
  session,
  includedCount,
  photoCount,
}: {
  session: ChatSessionSummary;
  includedCount: number;
  photoCount: number;
}) {
  const warnings = session.warnings.filter((w) => w.kind === "warning");
  const infos = session.warnings.filter((w) => w.kind === "info");
  const videos = session.media.filter((m) => m.kind === "video").length;

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">Diagnóstico del ZIP</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap gap-1.5">
          <Badge variant="outline">
            {session.format === "bracket"
              ? "Android"
              : session.format === "dash"
                ? "iOS"
                : "Formato mixto"}
          </Badge>
          <Badge variant="secondary">{session.messageCount} mensajes</Badge>
          <Badge variant="success">
            {includedCount}/{session.novedades.length} incluidas
          </Badge>
          <Badge variant="secondary">
            <ImageIcon className="h-3 w-3" /> {photoCount}
          </Badge>
          {videos > 0 ? (
            <Badge variant="warning">
              <Video className="h-3 w-3" /> {videos}
            </Badge>
          ) : null}
        </div>

        {warnings.map((warning, index) => (
          <Alert key={`w-${index}`} variant="warning">
            {warning.message}
          </Alert>
        ))}

        {infos.length > 0 ? (
          <details className="rounded-lg border border-border/60 px-3 py-2">
            <summary className="cursor-pointer text-xs font-medium text-muted-foreground">
              Detalles del análisis ({infos.length})
            </summary>
            <div className="mt-2 space-y-2">
              {infos.map((info, index) => (
                <div key={`i-${index}`} className="flex gap-2 text-xs text-muted-foreground">
                  <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                  <span>{info.message}</span>
                </div>
              ))}
            </div>
          </details>
        ) : null}
      </CardContent>
    </Card>
  );
}

function HowItWorks() {
  const steps = [
    "Cargar el .zip del chat (con multimedia).",
    "Revisar y editar las novedades detectadas.",
    "Formalizar con IA o limpiar sin conexión.",
    "Verificar el rango de fechas y generar el .docx.",
  ];
  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">Flujo de trabajo</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {steps.map((step, index) => (
          <div key={step} className="flex items-start gap-3">
            <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/15 text-[11px] font-semibold text-primary">
              {index + 1}
            </div>
            <p className="text-sm text-muted-foreground">{step}</p>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

/* ------------------------------------------------------------------ */
/* Lista de novedades                                                  */
/* ------------------------------------------------------------------ */

function novedadTitle(n: Novedad): string {
  const header = n.headers.find((h) => h.trim());
  if (header) return header;
  const line = n.bodyRaw.split("\n").find((l) => l.trim());
  if (line) return truncate(line, 70);
  return `Novedad #${n.id}`;
}

function NovedadList({
  novedades,
  total,
  selectedId,
  filter,
  onFilter,
  onSelect,
  onUpdate,
  onSetAll,
}: {
  novedades: Novedad[];
  total: number;
  selectedId: number | null;
  filter: string;
  onFilter: (value: string) => void;
  onSelect: (id: number) => void;
  onUpdate: (id: number, patch: Partial<Novedad>) => void;
  onSetAll: (include: boolean) => void;
}) {
  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between gap-2">
          <CardTitle className="text-sm">Novedades ({total})</CardTitle>
          <div className="flex gap-1">
            <Button variant="ghost" size="icon" title="Marcar todas" onClick={() => onSetAll(true)}>
              <CheckCheck className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              title="Desmarcar todas"
              onClick={() => onSetAll(false)}
            >
              <CircleSlash className="h-4 w-4" />
            </Button>
          </div>
        </div>
        <Input
          value={filter}
          onChange={(event) => onFilter(event.target.value)}
          placeholder="Filtrar por texto, encabezado o remitente…"
          className="mt-2 h-8 text-xs"
        />
      </CardHeader>
      <CardContent className="p-2 pt-0">
        <div className="max-h-[calc(100vh-430px)] space-y-1.5 overflow-y-auto pr-1">
          {novedades.length === 0 ? (
            <p className="px-2 py-6 text-center text-xs text-muted-foreground">
              Sin resultados para el filtro.
            </p>
          ) : (
            novedades.map((n) => {
              const active = n.id === selectedId;
              return (
                <div
                  key={n.id}
                  role="button"
                  tabIndex={0}
                  onClick={() => onSelect(n.id)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") onSelect(n.id);
                  }}
                  className={cn(
                    "cursor-pointer rounded-lg border px-3 py-2.5 transition-colors",
                    active
                      ? "border-primary/50 bg-primary/10"
                      : "border-border/60 bg-card/40 hover:bg-accent/40",
                    !n.include && "opacity-60",
                  )}
                >
                  <div className="flex items-start gap-2">
                    <span className="pt-0.5" onClick={(event) => event.stopPropagation()}>
                      <Checkbox
                        checked={n.include}
                        onChange={(event) => onUpdate(n.id, { include: event.target.checked })}
                      />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5">
                        <span className="text-[11px] font-semibold text-muted-foreground">
                          #{n.id}
                        </span>
                        <span className="truncate text-xs font-medium">{novedadTitle(n)}</span>
                      </div>
                      <p className="mt-0.5 line-clamp-2 text-[11px] leading-snug text-muted-foreground">
                        {truncate(n.bodyFormal || n.bodyRaw, 130) || "(sin texto)"}
                      </p>
                      <div className="mt-1.5 flex flex-wrap items-center gap-1">
                        {n.fechaMencionada ? (
                          <Badge variant="outline">{formatDate(n.fechaMencionada)}</Badge>
                        ) : null}
                        {n.photos.length > 0 ? (
                          <Badge variant="default">
                            <ImageIcon className="h-3 w-3" /> {n.photos.length}
                          </Badge>
                        ) : null}
                        {n.videosSkipped.length > 0 ? (
                          <Badge variant="warning">
                            <Video className="h-3 w-3" /> {n.videosSkipped.length}
                          </Badge>
                        ) : null}
                        {n.isOrphan ? <Badge variant="destructive">Huérfana</Badge> : null}
                        {n.include ? (
                          <Badge variant="success">
                            <Check className="h-3 w-3" /> Incluida
                          </Badge>
                        ) : null}
                      </div>
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </CardContent>
    </Card>
  );
}

/* ------------------------------------------------------------------ */
/* Editor                                                              */
/* ------------------------------------------------------------------ */

function NovedadEditor({
  sessionId,
  novedad,
  allNovedades,
  busy,
  onUpdate,
  onFormalize,
  onCleanup,
}: {
  sessionId: string;
  novedad: Novedad;
  allNovedades: Novedad[];
  busy: boolean;
  onUpdate: (patch: Partial<Novedad>) => void;
  onFormalize: () => void;
  onCleanup: () => void;
}) {
  const movePhotos = useWorkspace((s) => s.movePhotos);
  const [targetId, setTargetId] = useState<number | "">("");

  const targets = allNovedades.filter((n) => n.id !== novedad.id);

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <CardTitle className="flex items-center gap-2 text-sm">
              Novedad #{novedad.id}
              {novedad.isOrphan ? <Badge variant="destructive">Fotos huérfanas</Badge> : null}
            </CardTitle>
            <CardDescription className="mt-1">
              Remitente: {novedad.sender || "(sin remitente)"}
              {novedad.fechaMencionada
                ? ` · Fecha mencionada: ${formatDate(novedad.fechaMencionada)}`
                : ""}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">Incluir</span>
            <Switch
              checked={novedad.include}
              onCheckedChange={(checked) => onUpdate({ include: checked })}
            />
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-1.5">
          <Label>Encabezados institucionales (uno por línea)</Label>
          <Textarea
            rows={2}
            value={novedad.headers.join("\n")}
            onChange={(event) =>
              onUpdate({
                headers: event.target.value
                  .split("\n")
                  .map((h) => h.trim())
                  .filter((h) => h.length > 0),
              })
            }
            placeholder="OPERATIVO ANTI-RUIDO…"
          />
        </div>

        <div className="grid gap-4 lg:grid-cols-2">
          <div className="space-y-1.5">
            <Label>Texto crudo (WhatsApp)</Label>
            <Textarea
              rows={10}
              value={novedad.bodyRaw}
              onChange={(event) => onUpdate({ bodyRaw: event.target.value })}
              className="font-mono text-xs leading-relaxed"
            />
          </div>
          <div className="space-y-1.5">
            <div className="flex items-center justify-between gap-2">
              <Label>Texto formal (revisable)</Label>
              <div className="flex gap-1">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onCleanup}
                  disabled={busy || !novedad.bodyRaw.trim()}
                >
                  <CircleSlash className="h-3.5 w-3.5" /> Limpieza
                </Button>
                <Button size="sm" onClick={onFormalize} disabled={busy || !novedad.bodyRaw.trim()}>
                  <Sparkles className="h-3.5 w-3.5" /> Formalizar IA
                </Button>
              </div>
            </div>
            <Textarea
              rows={10}
              value={novedad.bodyFormal}
              onChange={(event) => onUpdate({ bodyFormal: event.target.value })}
              className="text-xs leading-relaxed"
              placeholder="El texto formal aparecerá aquí para su revisión…"
            />
          </div>
        </div>

        {novedad.closing ? (
          <Alert variant="info" title="Cierre detectado">
            {novedad.closing}
          </Alert>
        ) : null}

        {novedad.photos.length > 0 ? (
          <div className="space-y-2">
            <Label className="flex items-center gap-2">
              <ImageIcon className="h-4 w-4" /> Fotos asociadas ({novedad.photos.length})
            </Label>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
              {novedad.photos.map((photo) => (
                <a
                  key={photo}
                  href={`/api/sessions/${sessionId}/media/${encodeURIComponent(
                    photo.toLowerCase(),
                  )}`}
                  target="_blank"
                  rel="noreferrer"
                  className="group relative aspect-video overflow-hidden rounded-lg border border-border/70 bg-muted"
                  title={photo}
                >
                  <img
                    src={`/api/sessions/${sessionId}/media/${encodeURIComponent(
                      photo.toLowerCase(),
                    )}`}
                    alt={photo}
                    loading="lazy"
                    className="h-full w-full object-cover transition-transform group-hover:scale-105"
                  />
                  <div className="absolute inset-x-0 bottom-0 truncate bg-black/60 px-2 py-1 text-[10px] text-white">
                    {photo}
                  </div>
                </a>
              ))}
            </div>
          </div>
        ) : null}

        {novedad.videosSkipped.length > 0 ? (
          <div className="space-y-1.5">
            <Label className="flex items-center gap-2">
              <Video className="h-4 w-4" /> Videos omitidos del .docx
            </Label>
            <div className="flex flex-wrap gap-1.5">
              {novedad.videosSkipped.map((video) => (
                <Badge key={video} variant="warning">
                  {video}
                </Badge>
              ))}
            </div>
          </div>
        ) : null}

        {novedad.isOrphan ? (
          <div className="space-y-2 rounded-lg border border-destructive/30 bg-destructive/5 p-3">
            <div className="flex items-center gap-2 text-sm font-medium">
              <ArrowRightLeft className="h-4 w-4" /> Reasignar fotos huérfanas
            </div>
            <p className="text-xs text-muted-foreground">
              Estas fotos llegaron sin texto asociado. Muévalas a la novedad correcta para que se
              incrusten en su tabla.
            </p>
            <div className="flex flex-wrap items-center gap-2">
              <Select
                value={targetId}
                onChange={(event) =>
                  setTargetId(event.target.value ? Number(event.target.value) : "")
                }
                className="h-8 max-w-xs text-xs"
              >
                <option value="">Seleccione la novedad destino…</option>
                {targets.map((n) => (
                  <option key={n.id} value={n.id}>
                    #{n.id} — {novedadTitle(n)}
                  </option>
                ))}
              </Select>
              <Button
                size="sm"
                variant="outline"
                disabled={targetId === ""}
                onClick={() => {
                  if (targetId === "") return;
                  movePhotos(novedad.id, Number(targetId));
                  toast.success("Fotos reasignadas.");
                  setTargetId("");
                }}
              >
                Mover fotos
              </Button>
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

/* ------------------------------------------------------------------ */
/* Barra de generación                                                 */
/* ------------------------------------------------------------------ */

function GenerateBar({
  dateFrom,
  dateTo,
  templateId,
  templates,
  includedCount,
  progress,
  generating,
  report,
  replaceMessage,
  onDates,
  onTemplate,
  onSave,
  saving,
  onGenerate,
  onConfirmReplace,
  onFormalizeAll,
  onCleanupAll,
}: {
  session: ChatSessionSummary;
  dateFrom: string;
  dateTo: string;
  templateId: string;
  templates: TemplateSummary[];
  includedCount: number;
  progress: { done: number; total: number } | null;
  generating: boolean;
  report: ReportResult | null;
  replaceMessage: string | null;
  onDates: (from: string, to: string) => void;
  onTemplate: (id: string) => void;
  onSave: () => void;
  saving: boolean;
  onGenerate: () => void;
  onConfirmReplace: () => void;
  onFormalizeAll: () => void;
  onCleanupAll: () => void;
}) {
  const previewName = `Parte_Diario_${formatDate(dateFrom)}_${formatDate(dateTo)}.docx`;

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-sm">
          <FileText className="h-4 w-4 text-primary" /> 2. Generar Parte Diario
        </CardTitle>
        <CardDescription>
          Se incluirán {includedCount} novedad(es). Las tablas antiguas de la plantilla se eliminan.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="dateFrom">Desde</Label>
            <Input
              id="dateFrom"
              type="date"
              value={dateFrom}
              onChange={(event) => onDates(event.target.value, dateTo)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="dateTo">Hasta</Label>
            <Input
              id="dateTo"
              type="date"
              value={dateTo}
              onChange={(event) => onDates(dateFrom, event.target.value)}
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <Label>Plantilla .docx</Label>
          <Select value={templateId} onChange={(event) => onTemplate(event.target.value)}>
            {templates.map((template) => (
              <option key={template.id} value={template.id}>
                {template.name}
                {template.isDefault ? " (predeterminada)" : ""}
              </option>
            ))}
          </Select>
          <p className="text-[11px] text-muted-foreground">
            Archivo de salida: <code>{previewName}</code>
          </p>
        </div>

        {replaceMessage ? (
          <Alert variant="warning" title="Ya existe un reporte para este rango">
            {replaceMessage}
            <div className="mt-2">
              <Button size="sm" variant="destructive" onClick={onConfirmReplace}>
                Reemplazar reporte
              </Button>
            </div>
          </Alert>
        ) : null}

        {report ? (
          <Alert variant="success" title="Documento generado">
            <div className="space-y-2">
              <p>
                {report.novedadesIncluidas} novedades y {report.imagesEmbedded} fotos incrustadas.
                Validación OOXML superada.
              </p>
              {report.warnings.length > 0 ? (
                <ul className="list-disc space-y-1 pl-4 text-xs">
                  {report.warnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              ) : null}
              <Button asChild size="sm" variant="success">
                <a href={report.downloadUrl}>
                  <Download className="h-4 w-4" /> Descargar .docx
                </a>
              </Button>
            </div>
          </Alert>
        ) : null}

        {progress ? (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>Procesando novedades…</span>
              <span>
                {progress.done}/{progress.total}
              </span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full bg-primary transition-all"
                style={{
                  width: `${
                    progress.total ? Math.round((progress.done / progress.total) * 100) : 0
                  }%`,
                }}
              />
            </div>
          </div>
        ) : null}

        <Separator />

        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={onFormalizeAll}
            disabled={!!progress || generating}
          >
            <Sparkles className="h-3.5 w-3.5" /> Formalizar TODAS (IA)
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={onCleanupAll}
            disabled={!!progress || generating}
          >
            <CircleSlash className="h-3.5 w-3.5" /> Limpiar TODAS
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={onSave}
            disabled={saving || !!progress || generating}
          >
            {saving ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Save className="h-3.5 w-3.5" />
            )}
            Guardar borrador
          </Button>
          <div className="ml-auto flex gap-2">
            <Button
              variant="secondary"
              onClick={onGenerate}
              disabled={generating || !!progress || includedCount === 0 || !dateFrom || !dateTo}
            >
              {generating ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Play className="h-4 w-4" />
              )}
              Generar Parte Diario
            </Button>
          </div>
        </div>

        <div className="flex items-start gap-2 rounded-lg bg-muted/40 px-3 py-2 text-[11px] text-muted-foreground">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--warning)]" />
          <span>
            El texto formal siempre se muestra para revisión. Si ya existe un reporte para este
            rango, se pedirá confirmación antes de reemplazarlo.
          </span>
        </div>
      </CardContent>
    </Card>
  );
}
