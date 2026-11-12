import { useEffect, useState } from "react";
import { Check, Copy01, Database01 } from "@untitledui/icons";
import { Heading as AriaHeading } from "react-aria-components";
import { Dialog, Modal, ModalOverlay } from "@/components/application/modals/modal";
import { AlertBanner } from "@/components/app/alert-banner";
import { Button } from "@/components/base/buttons/button";
import { ButtonUtility } from "@/components/base/buttons/button-utility";
import { CloseButton } from "@/components/base/buttons/close-button";
import { Input } from "@/components/base/input/input";
import { RadioButton, RadioGroup } from "@/components/base/radio-buttons/radio-buttons";
import { Toggle } from "@/components/base/toggle/toggle";
import { FeaturedIcon } from "@/components/foundations/featured-icon/featured-icon";
import { useI18n } from "@/i18n";
import { type AuthKind, type Connection, type ConnectionParams, type ServerInfo, api, errorMessage } from "@/lib/api";

const isWindows = navigator.userAgent.includes("Windows");

interface FormState {
    name: string;
    server: string;
    port: string;
    database: string;
    auth: AuthKind;
    username: string;
    /** `null` = untouched (keep the stored password when editing) */
    password: string | null;
    encrypt: boolean;
    trustServerCertificate: boolean;
}

const emptyForm: FormState = {
    name: "",
    server: "",
    port: "",
    database: "",
    auth: "sql",
    username: "",
    password: null,
    encrypt: true,
    trustServerCertificate: false,
};

const fromConnection = (c: Connection): FormState => ({
    name: c.name,
    server: c.server,
    port: c.port?.toString() ?? "",
    database: c.database ?? "",
    auth: c.auth,
    username: c.username ?? "",
    password: null,
    encrypt: c.encrypt,
    trustServerCertificate: c.trustServerCertificate,
});

const toParams = (f: FormState): ConnectionParams => {
    const port = Number.parseInt(f.port, 10);
    return {
        server: f.server.trim(),
        port: Number.isInteger(port) && port > 0 && port < 65536 ? port : null,
        database: f.database.trim() || null,
        auth: f.auth,
        username: f.auth === "sql" ? f.username.trim() || null : null,
        encrypt: f.encrypt,
        trustServerCertificate: f.trustServerCertificate,
    };
};

interface ConnectionDialogProps {
    isOpen: boolean;
    /** `null` = add new connection */
    connection: Connection | null;
    onClose: () => void;
    onSaved: (connection: Connection) => void;
}

export const ConnectionDialog = ({ isOpen, connection, onClose, onSaved }: ConnectionDialogProps) => {
    const { t } = useI18n();
    const [form, setForm] = useState<FormState>(emptyForm);
    const [preview, setPreview] = useState("");
    const [copied, setCopied] = useState(false);
    const [saving, setSaving] = useState(false);
    const [testing, setTesting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [testResult, setTestResult] = useState<ServerInfo | null>(null);

    useEffect(() => {
        if (!isOpen) return;
        setForm(connection ? fromConnection(connection) : emptyForm);
        setError(null);
        setTestResult(null);
    }, [isOpen, connection]);

    const set = <K extends keyof FormState>(key: K, value: FormState[K]) => {
        setForm((f) => ({ ...f, [key]: value }));
        setTestResult(null);
    };

    const willHavePassword = form.auth === "sql" && (form.password === null ? Boolean(connection?.hasPassword) : form.password !== "");

    // The read-only connection string is built by the backend (single source of truth).
    useEffect(() => {
        if (!isOpen) return;
        let cancelled = false;
        api.previewConnectionString(toParams(form), willHavePassword)
            .then((cs) => !cancelled && setPreview(cs))
            .catch(() => undefined);
        return () => {
            cancelled = true;
        };
    }, [form, isOpen, willHavePassword]);

    const canSubmit = form.name.trim() !== "" && form.server.trim() !== "" && (form.auth === "windows" || form.username.trim() !== "");

    const test = async () => {
        setTesting(true);
        setError(null);
        setTestResult(null);
        try {
            setTestResult(await api.testConnection(toParams(form), form.password, connection?.id ?? null));
        } catch (e) {
            setError(errorMessage(e));
        } finally {
            setTesting(false);
        }
    };

    const save = async () => {
        setSaving(true);
        setError(null);
        try {
            const input = { name: form.name, ...toParams(form), password: form.password };
            const saved = connection ? await api.updateConnection(connection.id, input) : await api.createConnection(input);
            onSaved(saved);
        } catch (e) {
            setError(errorMessage(e));
        } finally {
            setSaving(false);
        }
    };

    const copy = async () => {
        await navigator.clipboard.writeText(preview);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
    };

    return (
        <ModalOverlay isOpen={isOpen} onOpenChange={(open) => !open && onClose()} isDismissable>
            <Modal className="max-w-2xl">
                <Dialog>
                    <form
                        className="flex w-full flex-col"
                        onSubmit={(e) => {
                            e.preventDefault();
                            if (canSubmit) void save();
                        }}
                    >
                        <div className="relative flex items-start gap-4 px-6 pt-6">
                            <FeaturedIcon icon={Database01} color="brand" theme="light" size="lg" />
                            <div className="flex flex-col gap-1">
                                <AriaHeading slot="title" className="text-lg font-semibold text-primary">
                                    {connection ? t("conn.edit") : t("conn.new")}
                                </AriaHeading>
                                <p className="text-sm text-tertiary">{t("conn.subtitle")}</p>
                            </div>
                            <CloseButton onPress={onClose} size="md" className="absolute top-3 right-3" />
                        </div>

                        <div className="flex flex-col gap-4 px-6 pt-5">
                            <Input
                                label={t("conn.name")}
                                placeholder={t("conn.namePlaceholder")}
                                value={form.name}
                                onChange={(v) => set("name", v)}
                                isRequired
                                autoFocus
                            />

                            <div className="flex gap-3">
                                <Input
                                    label={t("conn.server")}
                                    placeholder={t("conn.serverPlaceholder")}
                                    value={form.server}
                                    onChange={(v) => set("server", v)}
                                    isRequired
                                    className="flex-1"
                                />
                                <Input
                                    label={t("conn.port")}
                                    placeholder="1433"
                                    inputMode="numeric"
                                    value={form.port}
                                    onChange={(v) => set("port", v.replace(/\D/g, "").slice(0, 5))}
                                    className="w-28"
                                />
                            </div>

                            <Input
                                label={t("conn.database")}
                                placeholder={t("conn.databasePlaceholder")}
                                value={form.database}
                                onChange={(v) => set("database", v)}
                            />

                            <RadioGroup
                                aria-label={t("conn.auth")}
                                value={form.auth}
                                onChange={(v) => set("auth", v as AuthKind)}
                                className="flex-row flex-wrap items-start gap-6"
                            >
                                <span className="text-sm font-medium text-secondary">{t("conn.auth")}:</span>
                                <RadioButton value="sql" label={t("conn.authSql")} />
                                <RadioButton
                                    value="windows"
                                    label={t("conn.authWindows")}
                                    hint={isWindows ? undefined : t("conn.authWindowsHint")}
                                    isDisabled={!isWindows && form.auth !== "windows"}
                                />
                            </RadioGroup>

                            {form.auth === "sql" && (
                                <div className="flex gap-3">
                                    <Input
                                        label={t("conn.user")}
                                        value={form.username}
                                        onChange={(v) => set("username", v)}
                                        isRequired
                                        className="flex-1"
                                        autoComplete="off"
                                    />
                                    <div className="flex flex-1 flex-col gap-1.5">
                                        <Input
                                            label={t("conn.password")}
                                            type="password"
                                            autoComplete="new-password"
                                            placeholder={connection?.hasPassword ? t("conn.passwordKeep") : undefined}
                                            value={form.password ?? ""}
                                            onChange={(v) => set("password", v)}
                                        />
                                        {connection?.hasPassword && form.password === null && (
                                            <Button color="link-gray" size="sm" className="self-start" onClick={() => set("password", "")}>
                                                {t("conn.passwordClear")}
                                            </Button>
                                        )}
                                    </div>
                                </div>
                            )}

                            <div className="grid grid-cols-2 gap-4">
                                <Toggle label={t("conn.encrypt")} hint={t("conn.encryptHint")} isSelected={form.encrypt} onChange={(v) => set("encrypt", v)} />
                                <Toggle
                                    label={t("conn.trust")}
                                    hint={t("conn.trustHint")}
                                    isSelected={form.trustServerCertificate}
                                    onChange={(v) => set("trustServerCertificate", v)}
                                />
                            </div>

                            <div className="flex flex-col gap-1.5">
                                <span className="text-sm font-medium text-secondary">{t("conn.connectionString")}</span>
                                <div className="flex items-start gap-2 rounded-lg bg-secondary px-3 py-2.5 ring-1 ring-secondary ring-inset">
                                    <code className="selectable flex-1 font-mono text-xs leading-5 break-all text-tertiary">{preview}</code>
                                    <ButtonUtility size="xs" color="tertiary" icon={copied ? Check : Copy01} tooltip={t("common.copy")} onClick={copy} />
                                </div>
                                <p className="text-xs text-tertiary">{t("conn.passwordHint")}</p>
                            </div>

                            {testResult && (
                                <AlertBanner tone="success" title={t("conn.testOk")}>
                                    {testResult.serverName} / {testResult.database} — SQL Server {testResult.productVersion}
                                    {testResult.majorVersion > 0 && testResult.majorVersion < 14 && (
                                        <p className="mt-1 font-medium text-warning-primary">{t("conn.oldVersion")}</p>
                                    )}
                                </AlertBanner>
                            )}
                            {error && (
                                <AlertBanner tone="error" title={t("common.error")} onDismiss={() => setError(null)}>
                                    {error}
                                </AlertBanner>
                            )}
                        </div>

                        <div className="flex flex-col-reverse gap-3 p-6 sm:flex-row sm:items-center">
                            <Button color="link-color" size="md" onClick={test} isLoading={testing} isDisabled={!canSubmit}>
                                {t("conn.test")}
                            </Button>
                            <div className="flex-1" />
                            <Button color="secondary" size="md" onClick={onClose}>
                                {t("common.cancel")}
                            </Button>
                            <Button type="submit" color="primary" size="md" isLoading={saving} isDisabled={!canSubmit}>
                                {t("common.save")}
                            </Button>
                        </div>
                    </form>
                </Dialog>
            </Modal>
        </ModalOverlay>
    );
};
