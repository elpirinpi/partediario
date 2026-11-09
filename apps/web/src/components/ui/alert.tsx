import * as React from "react";
import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const alertVariants = cva("relative flex w-full gap-3 rounded-lg border px-4 py-3 text-sm", {
  variants: {
    variant: {
      default: "border-border bg-card text-card-foreground",
      info: "border-primary/30 bg-primary/8 text-foreground",
      success: "border-[var(--success)]/30 bg-[var(--success)]/8 text-foreground",
      warning: "border-[var(--warning)]/35 bg-[var(--warning)]/8 text-foreground",
      destructive: "border-destructive/35 bg-destructive/8 text-foreground",
    },
  },
  defaultVariants: { variant: "default" },
});

const icons = {
  default: Info,
  info: Info,
  success: CheckCircle2,
  warning: AlertTriangle,
  destructive: XCircle,
} as const;

export interface AlertProps
  extends React.HTMLAttributes<HTMLDivElement>, VariantProps<typeof alertVariants> {
  title?: string;
}

function Alert({ className, variant = "default", title, children, ...props }: AlertProps) {
  const Icon = icons[variant ?? "default"];
  return (
    <div className={cn(alertVariants({ variant }), className)} {...props}>
      <Icon className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
      <div className="min-w-0 flex-1">
        {title ? <div className="mb-0.5 font-medium">{title}</div> : null}
        <div className="text-sm text-muted-foreground [&_strong]:text-foreground">{children}</div>
      </div>
    </div>
  );
}

export { Alert, alertVariants };
