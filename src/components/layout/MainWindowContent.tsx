import { cn } from '@/lib/utils'
import { ReviewSurface } from '@/components/secretariat/ReviewSurface'

interface MainWindowContentProps {
  children?: React.ReactNode
  className?: string
}

export function MainWindowContent({
  children,
  className,
}: MainWindowContentProps) {
  return (
    <div className={cn('flex h-full flex-col bg-background', className)}>
      {children ?? <ReviewSurface />}
    </div>
  )
}
