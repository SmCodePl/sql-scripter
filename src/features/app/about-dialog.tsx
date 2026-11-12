import type { ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Building07, Globe01, Mail01, User01 } from "@untitledui/icons";
import { Heading as AriaHeading } from "react-aria-components";
import { Dialog, Modal, ModalOverlay } from "@/components/application/modals/modal";
import { Button } from "@/components/base/buttons/button";
import { CloseButton } from "@/components/base/buttons/close-button";
import { useI18n } from "@/i18n";
import smcodeLogo from "@/assets/smcode-logo.png";

export const SMCODE = {
    company: "SmCode",
    author: "Sławomir Markiewicz",
    email: "support@sm-code.com",
    website: "https://sm-code.com",
};

interface AboutDialogProps {
    isOpen: boolean;
    version: string | null;
    onClose: () => void;
}

const Row = ({ icon: Icon, label, children }: { icon: typeof Mail01; label: string; children: ReactNode }) => (
    <div className="flex items-center gap-3 py-2.5">
        <Icon className="size-5 shrink-0 text-fg-quaternary" />
        <span className="w-36 shrink-0 text-sm text-tertiary">{label}</span>
        <span className="min-w-0 flex-1 text-sm font-medium text-primary">{children}</span>
    </div>
);

export const AboutDialog = ({ isOpen, version, onClose }: AboutDialogProps) => {
    const { t } = useI18n();

    return (
        <ModalOverlay isOpen={isOpen} onOpenChange={(open) => !open && onClose()} isDismissable>
            <Modal className="max-w-md">
                <Dialog>
                    <div className="relative flex w-full flex-col items-center gap-5 p-6 pt-8 text-center">
                        <CloseButton onPress={onClose} size="md" className="absolute top-3 right-3" />
                        <img src={smcodeLogo} alt="SmCode" className="h-16 w-auto" draggable={false} />
                        <div>
                            <AriaHeading slot="title" className="text-lg font-semibold text-primary">
                                SQL Scripter
                            </AriaHeading>
                            {version && <p className="text-sm text-tertiary">{t("about.version", { v: version })}</p>}
                            <p className="mt-2 text-sm text-tertiary">{t("about.description")}</p>
                        </div>

                        <div className="w-full divide-y divide-secondary border-y border-secondary text-left">
                            <Row icon={Building07} label={t("about.company")}>
                                {SMCODE.company}
                            </Row>
                            <Row icon={User01} label={t("about.author")}>
                                {SMCODE.author}
                            </Row>
                            <Row icon={Mail01} label={t("about.contact")}>
                                <Button color="link-color" size="sm" onClick={() => void openUrl(`mailto:${SMCODE.email}`)}>
                                    {SMCODE.email}
                                </Button>
                            </Row>
                            <Row icon={Globe01} label={t("about.website")}>
                                <Button color="link-color" size="sm" onClick={() => void openUrl(SMCODE.website)}>
                                    sm-code.com
                                </Button>
                            </Row>
                        </div>

                        <p className="text-xs text-quaternary">© {new Date().getFullYear()} SmCode. All rights reserved.</p>
                    </div>
                </Dialog>
            </Modal>
        </ModalOverlay>
    );
};
