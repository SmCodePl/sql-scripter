import { useState } from "react";
import { Trash01 } from "@untitledui/icons";
import { Heading as AriaHeading } from "react-aria-components";
import { Dialog, Modal, ModalOverlay } from "@/components/application/modals/modal";
import { Button } from "@/components/base/buttons/button";
import { FeaturedIcon } from "@/components/foundations/featured-icon/featured-icon";
import { useI18n } from "@/i18n";
import type { Connection } from "@/lib/api";

interface ConfirmDeleteDialogProps {
    connection: Connection | null;
    onCancel: () => void;
    onConfirm: (connection: Connection) => Promise<void>;
}

export const ConfirmDeleteDialog = ({ connection, onCancel, onConfirm }: ConfirmDeleteDialogProps) => {
    const { t } = useI18n();
    const [busy, setBusy] = useState(false);
    const [before, after] = t("delete.body").split("{name}");

    return (
        <ModalOverlay isOpen={connection !== null} onOpenChange={(open) => !open && onCancel()} isDismissable>
            <Modal className="max-w-md">
                <Dialog>
                    <div className="flex w-full flex-col gap-4 p-6">
                        <FeaturedIcon icon={Trash01} color="error" theme="light" size="lg" />
                        <div className="flex flex-col gap-1">
                            <AriaHeading slot="title" className="text-lg font-semibold text-primary">
                                {t("delete.title")}
                            </AriaHeading>
                            <p className="text-sm text-tertiary">
                                {before}
                                <span className="font-semibold text-secondary">{connection?.name}</span>
                                {after}
                            </p>
                        </div>
                        <div className="flex gap-3 pt-2">
                            <Button color="secondary" size="lg" className="flex-1" onClick={onCancel}>
                                {t("common.cancel")}
                            </Button>
                            <Button
                                color="primary-destructive"
                                size="lg"
                                className="flex-1"
                                isLoading={busy}
                                onClick={async () => {
                                    if (!connection) return;
                                    setBusy(true);
                                    try {
                                        await onConfirm(connection);
                                    } finally {
                                        setBusy(false);
                                    }
                                }}
                            >
                                {t("common.delete")}
                            </Button>
                        </div>
                    </div>
                </Dialog>
            </Modal>
        </ModalOverlay>
    );
};
