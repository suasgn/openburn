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

import { useDarkMode } from "@/hooks/use-dark-mode"
import { getRelativeLuminance } from "@/lib/color"
import { cn } from "@/lib/utils"

function GaugeIcon({ className }: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="currentColor"
      className={className}
    >
      <path d="M12 2C17.5228 2 22 6.47715 22 12C22 17.5228 17.5228 22 12 22C6.47715 22 2 17.5228 2 12C2 6.47715 6.47715 2 12 2ZM12 4C7.58172 4 4 7.58172 4 12C4 16.4183 7.58172 20 12 20C16.4183 20 20 16.4183 20 12C20 7.58172 16.4183 4 12 4ZM15.8329 7.33748C16.0697 7.17128 16.3916 7.19926 16.5962 7.40381C16.8002 7.60784 16.8267 7.92955 16.6587 8.16418C14.479 11.2095 13.2796 12.8417 13.0607 13.0607C12.4749 13.6464 11.5251 13.6464 10.9393 13.0607C10.3536 12.4749 10.3536 11.5251 10.9393 10.9393C11.3126 10.5661 12.9438 9.36549 15.8329 7.33748ZM17.5 11C18.0523 11 18.5 11.4477 18.5 12C18.5 12.5523 18.0523 13 17.5 13C16.9477 13 16.5 12.5523 16.5 12C16.5 11.4477 16.9477 11 17.5 11ZM6.5 11C7.05228 11 7.5 11.4477 7.5 12C7.5 12.5523 7.05228 13 6.5 13C5.94772 13 5.5 12.5523 5.5 12C5.5 11.4477 5.94772 11 6.5 11ZM8.81802 7.40381C9.20854 7.79433 9.20854 8.4275 8.81802 8.81802C8.4275 9.20854 7.79433 9.20854 7.40381 8.81802C7.01328 8.4275 7.01328 7.79433 7.40381 7.40381C7.79433 7.01328 8.4275 7.01328 8.81802 7.40381ZM12 5.5C12.5523 5.5 13 5.94772 13 6.5C13 7.05228 12.5523 7.5 12 7.5C11.4477 7.5 11 7.05228 11 6.5C11 5.94772 11.4477 5.5 12 5.5Z" />
    </svg>
  )
}

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
        <GaugeIcon className="size-6 dark:text-page-accent" />
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
