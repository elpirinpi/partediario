import { NavLink, useNavigate } from "react-router-dom";
import { History, LogOut, Settings, ShieldCheck, Volume2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { authClient } from "@/lib/auth-client";

interface AppShellProps {
  user: { name?: string | null; email?: string | null };
  children: React.ReactNode;
}

const navItems = [
  { to: "/", label: "Parte Diario", icon: Volume2 },
  { to: "/history", label: "Historial", icon: History },
  { to: "/settings", label: "Ajustes", icon: Settings },
];

export function AppShell({ user, children }: AppShellProps) {
  const navigate = useNavigate();

  const handleSignOut = async () => {
    await authClient.signOut();
    navigate("/login", { replace: true });
  };

  return (
    <div className="min-h-screen bg-background">
      <header className="sticky top-0 z-40 border-b border-border/70 bg-background/85 backdrop-blur">
        <div className="mx-auto flex h-14 max-w-[1600px] items-center gap-4 px-4 lg:px-6">
          <div className="flex items-center gap-2.5">
            <img
              src="/icono.png"
              alt="Parte Diario"
              className="h-8 w-8 rounded-lg object-cover ring-1 ring-border"
            />
            <div className="leading-tight">
              <div className="text-sm font-semibold">Parte Diario</div>
              <div className="text-[11px] text-muted-foreground">Anti-Ruidos · P.N.</div>
            </div>
          </div>

          <nav className="ml-4 flex items-center gap-1">
            {navItems.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.to === "/"}
                className={({ isActive }) =>
                  cn(
                    "flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
                    isActive
                      ? "bg-accent text-foreground"
                      : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                  )
                }
              >
                <item.icon className="h-4 w-4" />
                {item.label}
              </NavLink>
            ))}
          </nav>

          <div className="ml-auto flex items-center gap-3">
            <div className="hidden items-center gap-1.5 text-xs text-muted-foreground sm:flex">
              <ShieldCheck className="h-3.5 w-3.5 text-[var(--success)]" />
              Uso interno
            </div>
            <div className="hidden text-right sm:block">
              <div className="text-xs font-medium">{user.name}</div>
              <div className="text-[11px] text-muted-foreground">{user.email}</div>
            </div>
            <Button variant="ghost" size="icon" onClick={handleSignOut} title="Cerrar sesión">
              <LogOut className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-[1600px] px-4 py-5 lg:px-6">{children}</main>
    </div>
  );
}
