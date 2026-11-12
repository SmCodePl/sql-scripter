import { Lock01, Settings01 } from "@untitledui/icons";
import { Heading as AriaHeading } from "react-aria-components";
import { Dialog, Modal, ModalOverlay } from "@/components/application/modals/modal";
import { CloseButton } from "@/components/base/buttons/close-button";
import { RadioButton, RadioGroup } from "@/components/base/radio-buttons/radio-buttons";
import { FeaturedIcon } from "@/components/foundations/featured-icon/featured-icon";
import { LANGUAGES, type Language, useI18n } from "@/i18n";

interface SettingsDialogProps {
    isOpen: boolean;
    dbPath: string | null;
    onClose: () => void;
}

export const SettingsDialog = ({ isOpen, dbPath, onClose }: SettingsDialogProps) => {
    const { t, language, setLanguage } = useI18n();

    return (
        <ModalOverlay isOpen={isOpen} onOpenChange={(open) => !open && onClose()} isDismissable>
            <Modal className="max-w-lg">
                <Dialog>
                    <div className="relative flex w-full flex-col gap-6 p-6">
                        <div className="flex items-center gap-4">
                            <FeaturedIcon icon={Settings01} color="gray" theme="modern" size="lg" />
                            <AriaHeading slot="title" className="text-lg font-semibold text-primary">
                                {t("settings.title")}
                            </AriaHeading>
                        </div>
                        <CloseButton onPress={onClose} size="md" className="absolute top-3 right-3" />

                        <section className="flex flex-col gap-3">
                            <h3 className="text-sm font-semibold text-secondary">{t("settings.language")}</h3>
                            <RadioGroup aria-label={t("settings.language")} value={language} onChange={(v) => void setLanguage(v as Language)}>
                                {LANGUAGES.map((l) => (
                                    <RadioButton key={l.code} value={l.code} label={l.label} />
                                ))}
                            </RadioGroup>
                        </section>

                        <section className="flex flex-col gap-2 border-t border-secondary pt-5">
                            <h3 className="text-sm font-semibold text-secondary">{t("settings.storage")}</h3>
                            <code className="selectable rounded-lg bg-secondary px-3 py-2 font-mono text-xs break-all text-tertiary">{dbPath}</code>
                            <p className="flex gap-2 text-sm text-tertiary">
                                <Lock01 className="mt-0.5 size-4 shrink-0 text-fg-brand-primary" />
                                {t("settings.security")}
                            </p>
                        </section>
                    </div>
                </Dialog>
            </Modal>
        </ModalOverlay>
    );
};
