import { NavLink } from 'react-router-dom'
import docsConfig from '@/config/docs-config'

export default function DocsSidenav() {
  return (
    <nav className="py-4">
      {docsConfig.sections.map((section, sectionIndex) => (
        <div key={sectionIndex} className="mb-6">
          <h3 className="px-4 text-sm font-semibold text-slate-400 uppercase tracking-wider mb-2">
            {section.title}
          </h3>
          <div className="flex flex-col">
            {section.items.map((item, itemIndex) => (
              <NavLink
                key={itemIndex}
                to={item.to}
                end={item.to === '/docs'}
                className={({ isActive }) =>
                  `px-4 py-2 text-sm font-mono transition-colors ${
                    isActive
                      ? 'bg-emerald-500/20 text-emerald-400 border-r-2 border-emerald-400'
                      : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
                  }`
                }
              >
                {item.label}
              </NavLink>
            ))}
          </div>
        </div>
      ))}
    </nav>
  )
}
