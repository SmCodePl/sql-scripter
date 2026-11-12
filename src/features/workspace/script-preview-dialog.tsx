import { useState } from "react";
import { Check, Copy01 } from "@untitledui/icons";
import { Heading as AriaHeading } from "react-aria-components";
import { Dialog, Modal, ModalOverlay } from "@/components/application/modals/modal";
import { LoadingIndicator } from "@/components/application/loading-indicator/loading-indicator";
import { Button } from "@/components/base/buttons/button";
import { CloseButton } from "@/components/base/buttons/close-button";
import { useI18n } from "@/i18n";

export interface PreviewState {
    title: string;
    fileName: string;
    script: string | null;
    error: string | null;
}

interface ScriptPreviewDialogProps {
    preview: PreviewState | null;
    onClose: () => void;
}

export const ScriptPreviewDialog = ({ preview, onClose }: ScriptPreviewDialogProps) => {
    const { t } = useI18n();
    const [copied, setCopied] = useState(false);

    const copy = async () => {
        if (!preview?.script) return;
        await navigator.clipboard.writeText(preview.script);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
    };

    return (
        <ModalOverlay isOpen={preview !== null} onOpenChange={(open) => !open && onClose()} isDismissable>
            <Modal className="max-w-5xl">
                <Dialog>
                    <div className="flex h-[80vh] w-full flex-col">
                        <div className="relative flex items-center gap-3 border-b border-secondary px-6 py-4">
                            <div className="min-w-0 flex-1">
                                <AriaHeading slot="title" className="truncate text-lg font-semibold text-primary">
                                    {preview?.title}
                                </AriaHeading>
                                <p className="text-sm text-tertiary">{t("preview.file", { file: preview?.fileName ?? "" })}</p>
                            </div>
                            <Button size="sm" color="secondary" iconLeading={copied ? Check : Copy01} onClick={copy} isDisabled={!preview?.script}>
                                {copied ? t("common.copied") : t("common.copy")}
                            </Button>
                            <CloseButton onPress={onClose} size="md" />
                        </div>
                        <div className="min-h-0 flex-1 overflow-auto bg-secondary">
                            {preview?.script == null && preview?.error == null && (
                                <div className="flex h-full items-center justify-center">
                                    <LoadingIndicator type="line-spinner" size="md" label={t("preview.generating")} />
                                </div>
                            )}
                            {preview?.error && <p className="selectable p-6 text-sm text-error-primary">{preview.error}</p>}
                            {preview?.script && (
                                <pre className="p-6 font-mono text-xs leading-5 whitespace-pre text-secondary">{preview.script.replaceAll("\r\n", "\n")}</pre>
                            )}
                        </div>
                    </div>
                </Dialog>
            </Modal>
        </ModalOverlay>
    );
};
