import { DatabaseZap } from 'lucide-react'

import { useI18n } from '../../app/useI18n'

interface SidebarSourceManagerProps {
  onOpenManager: () => void
}

export function SidebarSourceManager({ onOpenManager }: SidebarSourceManagerProps) {
  const { t } = useI18n()

  return (
    <button className="nav-button sidebar-source-nav-button" onClick={onOpenManager} type="button">
      <DatabaseZap size={18} /> {t.sources.remoteSources}
    </button>
  )
}
