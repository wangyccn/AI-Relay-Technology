import { forwardRef } from 'react';
import { Skeleton } from './Skeleton';

export interface SkeletonTableProps {
  rows?: number;
  columns?: number;
  className?: string;
}

export const SkeletonTable = forwardRef<HTMLDivElement, SkeletonTableProps>(
  (
    {
      rows = 5,
      columns = 4,
      className = '',
    },
    ref
  ) => {
    const classNames = ['ccr-skeleton-table', className].filter(Boolean).join(' ');

    return (
      <div ref={ref} className={classNames} aria-hidden="true">
        {/* Header */}
        <div className="ccr-skeleton-table__header">
          {Array.from({ length: columns }).map((_, index) => (
            <div key={index} className="ccr-skeleton-table__cell">
              <Skeleton variant="text" width="80%" height={14} />
            </div>
          ))}
        </div>
        
        {/* Rows */}
        {Array.from({ length: rows }).map((_, rowIndex) => (
          <div key={rowIndex} className="ccr-skeleton-table__row">
            {Array.from({ length: columns }).map((_, colIndex) => (
              <div key={colIndex} className="ccr-skeleton-table__cell">
                <Skeleton
                  variant="text"
                  width={colIndex === 0 ? '90%' : '70%'}
                  height={14}
                />
              </div>
            ))}
          </div>
        ))}
      </div>
    );
  }
);

SkeletonTable.displayName = 'SkeletonTable';
