import { useState } from "react";
import { Heading as AriaHeading } from "react-aria-components";
import { Dialog, Modal, ModalOverlay } from "@/components/application/modals/modal";
import { Button } from "@/components/base/buttons/button";
import { CloseButton } from "@/components/base/buttons/close-button";
import { Input } from "@/components/base/input/input";
import { useI18n } from "@/i18n";

export interface PasswordPromptState {
    connectionId: number;
    connectionName: string;
}

interface PasswordPromptDialogProps {
    prompt: PasswordPromptState | null;
    isLoading: boolean;
    onSubmit: (connectionId: number, password: string) => void;
    onClose: () => void;
}

export const PasswordPromptDialog = ({ prompt, isLoading, onSubmit, onClose }: PasswordPromptDialogProps) => {
    const { t } = useI18n();
    const [password, setPassword] = useState("");

    const handleSubmit = () => {
        if (!prompt || !password) return;
        onSubmit(prompt.connectionId, password);
    };

    const handleOpenChange = (open: boolean) => {
        if (!open) {
            setPassword("");
            onClose();
        }
    };

    return (
        <ModalOverlay isOpen={prompt !== null} onOpenChange={handleOpenChange} isDismissable={!isLoading}>
            <Modal className="max-w-md">
                <Dialog>
                    <div className="flex flex-col gap-4 p-6">
                        <div className="flex items-start justify-between">
                            <div>
                                <AriaHeading slot="title" className="text-lg font-semibold text-primary">
                                    {t("ws.passwordTitle")}
                                </AriaHeading>
                                <p className="mt-1 text-sm text-tertiary">
                                    {t("ws.passwordBody", { name: prompt?.connectionName ?? "" })}
                                </p>
                            </div>
                            <CloseButton onPress={onClose} size="md" />
                        </div>
                        <form
                            onSubmit={(e) => {
                                e.preventDefault();
                                handleSubmit();
                            }}
                        >
                            <Input
                                label={t("ws.passwordLabel")}
                                type="password"
                                size="sm"
                                value={password}
                                onChange={setPassword}
                                autoFocus
                            />
                            <div className="mt-4 flex justify-end gap-3">
                                <Button size="sm" color="secondary" onClick={onClose} isDisabled={isLoading}>
                                    {t("common.cancel")}
                                </Button>
                                <Button size="sm" color="primary" type="submit" isLoading={isLoading} isDisabled={!password}>
                                    {t("ws.passwordConnect")}
                                </Button>
                            </div>
                        </form>
                    </div>
                </Dialog>
            </Modal>
        </ModalOverlay>
    );
};
