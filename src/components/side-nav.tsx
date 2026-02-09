import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core"
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { Settings } from "lucide-react"
import AppIcon from "~icons/mynaui/daze-square-solid"

import { useDarkMode } from "@/hooks/use-dark-mode"
import { getRelativeLuminance } from "@/lib/color"
import { cn } from "@/lib/utils"

type ActiveView = "home" | "settings" | string

interface NavProvider {
  id: string
  name: string
  iconUrl: string
  brandColor?: string
}

interface SideNavProps {
  activeView: ActiveView
  onViewChange: (view: ActiveView) => void
  providers: NavProvider[]
  onReorderProviders: (orderedProviderIds: string[]) => void
}

interface NavButtonProps {
  isActive: boolean
  onClick: () => void
  children: React.ReactNode
  "aria-label"?: string
}

function navButtonClass(isActive: boolean): string {
  return cn(
    "relative flex items-center justify-center w-full p-2.5 transition-colors",
    "hover:bg-accent",
    isActive
      ? "text-foreground before:absolute before:left-0 before:top-1.5 before:bottom-1.5 before:w-0.5 before:bg-primary dark:before:bg-page-accent before:rounded-full"
      : "text-muted-foreground",
  )
}

function NavButton({ isActive, onClick, children, "aria-label": ariaLabel }: NavButtonProps) {
  return (
    <button type="button" onClick={onClick} aria-label={ariaLabel} className={navButtonClass(isActive)}>
      {children}
    </button>
  )
}

function getIconColor(brandColor: string | undefined, isDark: boolean): string {
  if (!brandColor) return "currentColor"
  const luminance = getRelativeLuminance(brandColor)
  if (isDark && luminance < 0.15) return "#ffffff"
  if (!isDark && luminance > 0.85) return "currentColor"
  return brandColor
}

function ProviderIcon({ provider, isDark }: { provider: NavProvider; isDark: boolean }) {
  return (
    <span
      role="img"
      aria-label={provider.name}
      className="size-6 inline-block"
      style={{
        backgroundColor: getIconColor(provider.brandColor, isDark),
        WebkitMaskImage: `url(${provider.iconUrl})`,
        WebkitMaskSize: "contain",
        WebkitMaskRepeat: "no-repeat",
        WebkitMaskPosition: "center",
        maskImage: `url(${provider.iconUrl})`,
        maskSize: "contain",
        maskRepeat: "no-repeat",
        maskPosition: "center",
      }}
    />
  )
}

function SortableProviderNavButton({
  provider,
  activeView,
  onViewChange,
  isDark,
}: {
  provider: NavProvider
  activeView: ActiveView
  onViewChange: (view: ActiveView) => void
  isDark: boolean
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: provider.id,
  })

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  }

  return (
    <button
      ref={setNodeRef}
      style={style}
      type="button"
      aria-label={provider.name}
      className={cn(navButtonClass(activeView === provider.id), "touch-none", isDragging && "opacity-70")}
      onClick={() => onViewChange(provider.id)}
      {...attributes}
      {...listeners}
    >
      <ProviderIcon provider={provider} isDark={isDark} />
    </button>
  )
}

export function SideNav({ activeView, onViewChange, providers, onReorderProviders }: SideNavProps) {
  const isDark = useDarkMode()
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 8 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  )

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event
    if (!over || active.id === over.id) {
      return
    }

    const oldIndex = providers.findIndex((provider) => provider.id === active.id)
    const newIndex = providers.findIndex((provider) => provider.id === over.id)
    if (oldIndex < 0 || newIndex < 0) {
      return
    }

    const reordered = arrayMove(providers, oldIndex, newIndex)
    onReorderProviders(reordered.map((provider) => provider.id))
  }

  return (
    <nav className="flex flex-col w-12 border-r bg-muted/50 dark:bg-card py-3">
      <NavButton isActive={activeView === "home"} onClick={() => onViewChange("home")} aria-label="Home">
        <AppIcon className="size-6 dark:text-page-accent" />
      </NavButton>

      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={providers.map((provider) => provider.id)} strategy={verticalListSortingStrategy}>
          {providers.map((provider) => (
            <SortableProviderNavButton
              key={provider.id}
              provider={provider}
              activeView={activeView}
              onViewChange={onViewChange}
              isDark={isDark}
            />
          ))}
        </SortableContext>
      </DndContext>

      <div className="flex-1" />

      <NavButton
        isActive={activeView === "settings"}
        onClick={() => onViewChange("settings")}
        aria-label="Settings"
      >
        <Settings className="size-6" />
      </NavButton>
    </nav>
  )
}

export type { ActiveView, NavProvider }
