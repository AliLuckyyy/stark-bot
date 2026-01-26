import { NavLink } from 'react-router-dom'
import docsConfig from '@/config/docs-config'

export default function DocsSidenav() {
  return (
    <div className="p-6 w-full flex flex-col">
      <div className="flex flex-col text-lg text-slate-400" style={{ minWidth: '200px' }}>
        {docsConfig.navbar.items.map((item, index) => (
          <NavLink
            key={index}
            to={item.to}
            end={item.to === '/docs'}
            className={({ isActive }) =>
              `px-3 py-2 rounded-lg transition-colors ${
                isActive
                  ? 'bg-cyan-500/20 text-cyan-400'
                  : 'hover:text-slate-200 hover:bg-slate-700/50'
              }`
            }
          >
            {item.label}
          </NavLink>
        ))}
      </div>
    </div>
  )
}
