import { Routes, Route } from 'react-router-dom'
import Home from './pages/Home'

// Import markdown docs as components
import DocsOverview from './views/docs/overview.md'
import DocsGettingStarted from './views/docs/getting-started.md'
import DocsArchitecture from './views/docs/architecture.md'
import DocsApi from './views/docs/api.md'
import DocsTools from './views/docs/tools.md'
import DocsSkills from './views/docs/skills.md'
import DocsChannels from './views/docs/channels.md'
import DocsScheduling from './views/docs/scheduling.md'
import DocsMemories from './views/docs/memories.md'
import DocsConfiguration from './views/docs/configuration.md'

function App() {
  return (
    <Routes>
      <Route path="/" element={<Home />} />
      <Route path="/docs" element={<DocsOverview />} />
      <Route path="/docs/getting-started" element={<DocsGettingStarted />} />
      <Route path="/docs/architecture" element={<DocsArchitecture />} />
      <Route path="/docs/api" element={<DocsApi />} />
      <Route path="/docs/tools" element={<DocsTools />} />
      <Route path="/docs/skills" element={<DocsSkills />} />
      <Route path="/docs/channels" element={<DocsChannels />} />
      <Route path="/docs/scheduling" element={<DocsScheduling />} />
      <Route path="/docs/memories" element={<DocsMemories />} />
      <Route path="/docs/configuration" element={<DocsConfiguration />} />
    </Routes>
  )
}

export default App
