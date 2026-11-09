import { Navigate, Route, Routes } from "react-router-dom";
import { Loader2 } from "lucide-react";
import { useSession } from "@/lib/auth-client";
import { AppShell } from "@/components/app-shell";
import LoginPage from "@/pages/login-page";
import WorkspacePage from "@/pages/workspace-page";
import SettingsPage from "@/pages/settings-page";
import HistoryPage from "@/pages/history-page";

function FullScreenLoader() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-background">
      <div className="flex flex-col items-center gap-3 text-muted-foreground">
        <Loader2 className="h-6 w-6 animate-spin text-primary" />
        <p className="text-sm">Cargando sesión…</p>
      </div>
    </div>
  );
}

export default function App() {
  const { data: session, isPending } = useSession();

  if (isPending) return <FullScreenLoader />;

  if (!session) {
    return (
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="*" element={<Navigate to="/login" replace />} />
      </Routes>
    );
  }

  return (
    <Routes>
      <Route path="/login" element={<Navigate to="/" replace />} />
      <Route
        path="/"
        element={
          <AppShell user={session.user}>
            <WorkspacePage />
          </AppShell>
        }
      />
      <Route
        path="/settings"
        element={
          <AppShell user={session.user}>
            <SettingsPage />
          </AppShell>
        }
      />
      <Route
        path="/history"
        element={
          <AppShell user={session.user}>
            <HistoryPage />
          </AppShell>
        }
      />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
