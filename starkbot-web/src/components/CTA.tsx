import { Github, BookOpen } from 'lucide-react'
import { Link } from 'react-router-dom'

export function CTA() {
  return (
    <section className="py-20 px-6">
      <div className="max-w-4xl mx-auto text-center">
        <h2 className="text-3xl sm:text-4xl font-bold mb-6">
          Ready to Get Started?
        </h2>
        <p className="text-slate-400 text-lg mb-8 max-w-2xl mx-auto">
          Join the community and start building with StarkBot today. It's free, open source, and ready to use.
        </p>
        <div className="flex flex-col sm:flex-row gap-4 justify-center">
          <a
            href="https://github.com/ethereumdegen/stark-bot"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-3 px-8 py-4 bg-gradient-to-r from-stark-500 to-stark-600 hover:from-stark-400 hover:to-stark-500 text-white font-semibold rounded-xl transition-all duration-300 transform hover:scale-105 shadow-lg hover:shadow-stark-500/25"
          >
            <Github className="w-6 h-6" />
            Star on GitHub
          </a>
          <Link
            to="/docs"
            className="inline-flex items-center gap-3 px-8 py-4 bg-slate-800 hover:bg-slate-700 text-white font-semibold rounded-xl transition-all duration-300 border border-slate-700 hover:border-stark-500"
          >
            <BookOpen className="w-6 h-6" />
            Read the Docs
          </Link>
        </div>
      </div>
    </section>
  )
}
