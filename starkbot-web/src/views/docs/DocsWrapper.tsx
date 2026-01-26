import type { ReactNode } from 'react'
import { NavLink } from 'react-router-dom'
import { ArrowLeft } from 'lucide-react'
import DocsSidenav from './DocsSidenav'

interface Props {
  attributes: Record<string, unknown>
  children: ReactNode
}

export default function DocsWrapper({ attributes, children }: Props) {
  return (
    <div className="min-h-screen bg-slate-900">
      {/* Top nav */}
      <header className="border-b border-slate-700 bg-slate-800/50 backdrop-blur-sm sticky top-0 z-10">
        <div className="max-w-7xl mx-auto px-4 h-16 flex items-center justify-between">
          <NavLink to="/" className="flex items-center gap-2 text-slate-400 hover:text-white transition-colors">
            <ArrowLeft className="w-4 h-4" />
            <span>Back to Home</span>
          </NavLink>
          <div className="text-xl font-bold text-cyan-400">StarkBot Docs</div>
        </div>
      </header>

      <div className="flex max-w-7xl mx-auto">
        {/* Docs Sidebar */}
        <aside className="w-64 shrink-0 border-r border-slate-700 sticky top-16 h-[calc(100vh-4rem)] overflow-y-auto hidden lg:block">
          <div className="p-4 border-b border-slate-700">
            <h2 className="text-lg font-semibold text-slate-200">Documentation</h2>
            <p className="text-sm text-slate-400">StarkBot Reference</p>
          </div>
          <DocsSidenav />
        </aside>

        {/* Docs Content */}
        <main className="flex-1 p-8 overflow-auto min-h-[calc(100vh-4rem)]">
          <div className="max-w-4xl markdown-body">
            {typeof attributes.name === 'string' && (
              <h1 className="text-3xl font-bold text-slate-100 mb-6 pb-2 border-b border-slate-700">
                {attributes.name}
              </h1>
            )}
            {children}
          </div>
        </main>
      </div>
    </div>
  )
}
