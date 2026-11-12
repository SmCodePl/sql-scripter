import type { ReactNode } from "react";
import { AlertCircle, CheckCircle, InfoCircle, XClose } from "@untitledui/icons";
import { Button as AriaButton } from "react-aria-components";
import { cx } from "@/utils/cx";

export type AlertTone = "error" | "success" | "info";

const tones: Record<AlertTone, { root: string; icon: string; Icon: typeof AlertCircle }> = {
    error: { root: "bg-error-primary ring-error_subtle", icon: "text-fg-error-primary", Icon: AlertCircle },
    success: { root: "bg-success-primary ring-secondary", icon: "text-fg-success-primary", Icon: CheckCircle },
    info: { root: "bg-secondary ring-secondary", icon: "text-fg-brand-primary", Icon: InfoCircle },
};

interface AlertBannerProps {
    tone: AlertTone;
    title: ReactNode;
    children?: ReactNode;
    onDismiss?: () => void;
    className?: string;
}

/** Inline alert in Untitled UI style (the free set has no alert component). */
export const AlertBanner = ({ tone, title, children, onDismiss, className }: AlertBannerProps) => {
    const { root, icon, Icon } = tones[tone];
    return (
        <div role={tone === "error" ? "alert" : "status"} className={cx("flex gap-3 rounded-xl p-4 ring-1 ring-inset", root, className)}>
            <Icon className={cx("mt-0.5 size-5 shrink-0", icon)} />
            <div className="min-w-0 flex-1 text-sm">
                <p className="font-semibold text-primary">{title}</p>
                {children && <div className="selectable mt-1 break-words text-secondary">{children}</div>}
            </div>
            {onDismiss && (
                <AriaButton aria-label="Zamknij" onPress={onDismiss} className="h-max rounded-md p-0.5 text-fg-quaternary outline-focus-ring hover:text-fg-quaternary_hover">
                    <XClose className="size-5" />
                </AriaButton>
            )}
        </div>
    );
};
