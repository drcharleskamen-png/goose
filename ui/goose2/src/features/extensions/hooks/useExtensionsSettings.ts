import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  addExtension,
  listExtensions,
  removeExtension,
  setExtensionEnabled,
} from "../api/extensions";
import { nameToKey } from "../lib/extensionKeys";
import {
  getDisplayName,
  type ExtensionConfig,
  type ExtensionEntry,
} from "../types";

type ExtensionModalMode = "add" | "edit" | null;

export function useExtensionsSettings() {
  const { t } = useTranslation("settings");
  const [extensions, setExtensions] = useState<ExtensionEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [modalMode, setModalMode] = useState<ExtensionModalMode>(null);
  const [editingExtension, setEditingExtension] =
    useState<ExtensionEntry | null>(null);
  const toggleVersions = useRef<Record<string, number>>({});

  const fetchExtensions = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await listExtensions();
      setExtensions(result);
    } catch {
      setExtensions([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchExtensions();
  }, [fetchExtensions]);

  const handleAdd = useCallback(() => {
    setEditingExtension(null);
    setModalMode("add");
  }, []);

  const handleConfigure = useCallback((extension: ExtensionEntry) => {
    setEditingExtension(extension);
    setModalMode("edit");
  }, []);

  const handleSubmit = useCallback(
    async (name: string, config: ExtensionConfig) => {
      try {
        const newKey = nameToKey(name);
        const isEdit = !!editingExtension;
        const isAdd = !editingExtension;
        const keyChanged = isEdit && editingExtension.config_key !== newKey;

        if (
          (isAdd || keyChanged) &&
          extensions.some((extension) => extension.config_key === newKey)
        ) {
          toast.error(t("extensions.errors.nameConflict", { name }));
          return;
        }

        await addExtension(name, config);
        if (keyChanged) {
          await removeExtension(editingExtension.config_key);
        }
        setModalMode(null);
        setEditingExtension(null);
        await fetchExtensions();
      } catch {
        toast.error(t("extensions.errors.saveFailed"));
      }
    },
    [editingExtension, extensions, fetchExtensions, t],
  );

  const handleDelete = useCallback(
    async (configKey: string) => {
      try {
        await removeExtension(configKey);
        setModalMode(null);
        setEditingExtension(null);
        await fetchExtensions();
      } catch {
        toast.error(t("extensions.errors.deleteFailed"));
      }
    },
    [fetchExtensions, t],
  );

  const handleToggleEnabled = useCallback(
    async (extension: ExtensionEntry, enabled: boolean) => {
      const configKey = extension.config_key;
      const version = (toggleVersions.current[configKey] ?? 0) + 1;
      toggleVersions.current[configKey] = version;

      setExtensions((current) =>
        current.map((item) =>
          item.config_key === configKey ? { ...item, enabled } : item,
        ),
      );

      try {
        await setExtensionEnabled(configKey, enabled);
        if (toggleVersions.current[configKey] === version) {
          await fetchExtensions();
        }
      } catch {
        if (toggleVersions.current[configKey] !== version) return;
        setExtensions((current) =>
          current.map((item) =>
            item.config_key === configKey
              ? { ...item, enabled: extension.enabled }
              : item,
          ),
        );
        toast.error(
          t("extensions.errors.toggleFailed", {
            name: getDisplayName(extension),
          }),
        );
      }
    },
    [fetchExtensions, t],
  );

  const handleModalClose = useCallback(() => {
    setModalMode(null);
    setEditingExtension(null);
  }, []);

  return {
    extensions,
    isLoading,
    modalMode,
    editingExtension,
    handleAdd,
    handleConfigure,
    handleSubmit,
    handleDelete,
    handleToggleEnabled,
    handleModalClose,
  };
}
