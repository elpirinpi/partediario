import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { Download, FileText, History as HistoryIcon, Image, Loader2 } from "lucide-react";
import type { HistoryEntry } from "@parte/shared";
import { api } from "@/lib/api";
import { formatDate, formatDateTime } from "@/lib/format";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/misc";

interface HistoryResponse {
  entries: HistoryEntry[];
  lastEnd: string | null;
}

interface ReportItem {
  id: string;
  filename: string;
  reportStart: string;
  reportEnd: string;
  novedades: number;
  images: number;
  createdAt: string;
  downloadUrl: string;
}

export default function HistoryPage() {
  const historyQuery = useQuery({
    queryKey: ["history"],
    queryFn: () => api<HistoryResponse>("/api/history"),
  });
  const reportsQuery = useQuery({
    queryKey: ["reports"],
    queryFn: () => api<ReportItem[]>("/api/reports"),
  });

  return (
    <div className="mx-auto max-w-5xl space-y-5">
      <div>
        <h2 className="text-lg font-semibold tracking-tight">Historial y control</h2>
        <p className="text-sm text-muted-foreground">
          Cada ZIP procesado queda registrado por SHA-256 para detectar duplicados y pre-seleccionar
          solo lo nuevo.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FileText className="h-4 w-4 text-primary" /> Reportes generados
          </CardTitle>
          <CardDescription>Descargue el .docx validado o consulte sus métricas.</CardDescription>
        </CardHeader>
        <CardContent>
          {reportsQuery.isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-14 w-full" />
              <Skeleton className="h-14 w-full" />
            </div>
          ) : (reportsQuery.data ?? []).length === 0 ? (
            <EmptyState text="Aún no hay reportes generados." />
          ) : (
            <div className="divide-y divide-border/60">
              {(reportsQuery.data ?? []).map((report) => (
                <div key={report.id} className="flex flex-wrap items-center gap-3 py-3">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">{report.filename}</div>
                    <div className="text-[11px] text-muted-foreground">
                      {formatDateTime(report.createdAt)}
                    </div>
                  </div>
                  <Badge variant="outline">
                    {formatDate(report.reportStart)} → {formatDate(report.reportEnd)}
                  </Badge>
                  <Badge variant="secondary">{report.novedades} novedades</Badge>
                  <Badge variant="secondary">
                    <Image className="h-3 w-3" /> {report.images}
                  </Badge>
                  <Button asChild variant="outline" size="sm">
                    <a href={report.downloadUrl}>
                      <Download className="h-4 w-4" /> Descargar
                    </a>
                  </Button>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HistoryIcon className="h-4 w-4 text-primary" /> ZIP procesados
          </CardTitle>
          <CardDescription>
            {historyQuery.data?.lastEnd
              ? `Último corte registrado: ${formatDate(historyQuery.data.lastEnd)}.`
              : "Sin cortes previos registrados."}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {historyQuery.isLoading ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" /> Cargando…
            </div>
          ) : (historyQuery.data?.entries ?? []).length === 0 ? (
            <EmptyState text="Aún no se ha procesado ningún ZIP." />
          ) : (
            <div className="divide-y divide-border/60">
              {(historyQuery.data?.entries ?? []).map((entry) => (
                <div key={entry.id} className="flex flex-wrap items-center gap-3 py-3">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">
                      {entry.zipName || entry.filename}
                    </div>
                    <div className="text-[11px] text-muted-foreground">
                      {formatDateTime(entry.generatedAt)}
                    </div>
                  </div>
                  <Badge variant="outline">
                    {formatDate(entry.reportStart)} → {formatDate(entry.reportEnd)}
                  </Badge>
                  <Badge variant="secondary">{entry.novedades} novedades</Badge>
                </div>
              ))}
            </div>
          )}
          <div className="pt-4">
            <Button asChild variant="link" className="px-0">
              <Link to="/">Volver al flujo principal</Link>
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function EmptyState({ text }: { text: string }) {
  return (
    <div className="rounded-lg border border-dashed border-border/70 px-4 py-8 text-center text-sm text-muted-foreground">
      {text}
    </div>
  );
}
