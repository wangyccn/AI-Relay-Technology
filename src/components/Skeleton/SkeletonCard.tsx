import { forwardRef } from 'react';
import { Skeleton } from './Skeleton';

export interface SkeletonCardProps {
  hasHeader?: boolean;
  hasImage?: boolean;
  lines?: number;
  className?: string;
}

export const SkeletonCard = forwardRef<HTMLDivElement, SkeletonCardProps>(
  (
    {
      hasHeader = true,
      hasImage = false,
      lines = 3,
      className = '',
    },
    ref
  ) => {
    const classNames = ['ccr-skeleton-card', className].filter(Boolean).join(' ');

    return (
      <div ref={ref} className={classNames} aria-hidden="true">
        {hasImage && (
          <Skeleton variant="rectangular" width="100%" height={160} />
        )}
        
        {hasHeader && (
          <div className="ccr-skeleton-card__header">
            <Skeleton variant="circular" width={40} height={40} />
            <div className="ccr-skeleton-card__header-text">
              <Skeleton variant="text" width="60%" height={16} />
              <Skeleton variant="text" width="40%" height={12} />
            </div>
          </div>
        )}
        
        <div className="ccr-skeleton-card__body">
          {Array.from({ length: lines }).map((_, index) => (
            <Skeleton
              key={index}
              variant="text"
              width={index === lines - 1 ? '70%' : '100%'}
              height={14}
            />
          ))}
        </div>
      </div>
    );
  }
);

SkeletonCard.displayName = 'SkeletonCard';
