/** Tipos compartidos entre la API y la interfaz web. */

export type ChatFormat = "bracket" | "dash" | "unknown";
export type AttachmentKind = "image" | "video" | "other";
export type LlmProvider = "openrouter" | "anthropic" | "freellmapi";

export interface Attachment {
  filename: string;
  kind: AttachmentKind;
}

export interface ChatMessage {
  ts: string | null;
  sender: string;
  text: string;
  attachment: Attachment | null;
  isSystem: boolean;
}

export interface ParsedChat {
  messages: ChatMessage[];
  format: ChatFormat;
}

export interface Novedad {
  id: number;
  headers: string[];
  bodyRaw: string;
  closing: string | null;
  photos: string[];
  videosSkipped: string[];
  fechaMencionada: string | null;
  include: boolean;
  isOrphan: boolean;
  bodyFormal: string;
  sender: string;
}

export interface SessionWarning {
  kind: "warning" | "info";
  message: string;
}

export interface MediaFile {
  name: string;
  size: number;
  kind: AttachmentKind;
}

export interface ChatSessionSummary {
  id: string;
  zipName: string;
  zipSha256: string;
  txtSha256: string;
  txtName: string;
  format: ChatFormat;
  messageCount: number;
  createdAt: string;
  dateFrom: string | null;
  dateTo: string | null;
  novedades: Novedad[];
  media: MediaFile[];
  warnings: SessionWarning[];
  duplicateOf: {
    reportStart: string;
    reportEnd: string;
    generatedAt: string;
    novedades: number;
  } | null;
}

export interface TemplateSummary {
  id: string;
  name: string;
  filename: string;
  isDefault: boolean;
  createdAt: string;
}

export interface AppSettings {
  provider: LlmProvider;
  model: string;
  apiKeySet: boolean;
  apiKeyPreview: string;
  anonymize: boolean;
  defaultTemplateId: string | null;
}

export interface HistoryEntry {
  id: string;
  zipName: string;
  reportStart: string;
  reportEnd: string;
  generatedAt: string;
  novedades: number;
  filename: string;
}

export interface ReportResult {
  id: string;
  filename: string;
  reportStart: string;
  reportEnd: string;
  novedadesIncluidas: number;
  imagesEmbedded: number;
  warnings: string[];
  downloadUrl: string;
}

export interface FormalizeRequest {
  ids: number[];
  useAi: boolean;
}

export interface FormalizeResult {
  id: number;
  bodyFormal: string;
  error: string | null;
}

export interface GenerateRequest {
  dateFrom: string;
  dateTo: string;
  templateId?: string | null;
  confirmReplace?: boolean;
  novedades: Novedad[];
}

export interface AuthUser {
  id: string;
  name: string;
  email: string;
  image?: string | null;
}
