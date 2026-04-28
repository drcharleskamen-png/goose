import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/cn";
import { Skeleton } from "@/shared/ui/skeleton";
import type { Persona } from "@/shared/types/agents";
import { PersonaCard } from "@/features/agents/ui/PersonaCard";

interface PersonaGalleryProps {
  personas: Persona[];
  activePersonaId?: string;
  onSelectPersona: (persona: Persona) => void;
  onEditPersona: (persona: Persona) => void;
  onDuplicatePersona: (persona: Persona) => void;
  onDeletePersona: (persona: Persona) => void;
  onExportPersona?: (persona: Persona) => void;
  isLoading?: boolean;
  dropHandlers?: React.HTMLAttributes<HTMLElement>;
  isDragOver?: boolean;
}

function SkeletonCard() {
  return (
    <div aria-hidden="true" className="flex w-[200px] shrink-0 flex-col py-4">
      <div className="flex h-[420px] w-full items-end justify-center">
        <Skeleton className="h-full w-[140px]" />
      </div>
      <Skeleton className="mt-4 h-px w-full" />
      <Skeleton className="mt-4 h-[144px] w-full" />
    </div>
  );
}

export function PersonaGallery({
  personas,
  activePersonaId,
  onSelectPersona,
  onEditPersona,
  onDuplicatePersona,
  onDeletePersona,
  onExportPersona,
  isLoading = false,
  dropHandlers,
  isDragOver = false,
}: PersonaGalleryProps) {
  const { t } = useTranslation("agents");
  const sorted = useMemo(() => {
    const builtins = personas
      .filter((p) => p.isBuiltin)
      .sort((a, b) => a.displayName.localeCompare(b.displayName));
    const custom = personas
      .filter((p) => !p.isBuiltin)
      .sort((a, b) => a.displayName.localeCompare(b.displayName));
    return [...builtins, ...custom];
  }, [personas]);

  if (isLoading) {
    return (
      <section
        role="status"
        aria-label={t("gallery.loading")}
        className="flex h-full items-center gap-32 overflow-x-auto px-16"
      >
        <SkeletonCard />
        <SkeletonCard />
        <SkeletonCard />
        <SkeletonCard />
      </section>
    );
  }

  return (
    <section
      {...dropHandlers}
      className={cn(
        "flex h-full items-center gap-32 overflow-x-auto px-16 transition-colors",
        isDragOver && "ring-2 ring-ring ring-offset-2 ring-inset",
      )}
    >
      {sorted.map((persona) => (
        <PersonaCard
          key={persona.id}
          persona={persona}
          isActive={persona.id === activePersonaId}
          onSelect={onSelectPersona}
          onEdit={onEditPersona}
          onDuplicate={onDuplicatePersona}
          onDelete={onDeletePersona}
          onExport={onExportPersona}
        />
      ))}
    </section>
  );
}
