import { forwardRef, useEffect, useState } from 'react';
import { Skeleton } from '../Skeleton';

export interface StatCardProps {
  title: string;
  value: string | number;
  unit?: string;
  icon?: React.ReactNode;
  trend?: {
    value: number;
    direction: 'up' | 'down';
  };
  loading?: boolean;
  details?: Array<{ label: string; value: string | number }>;
  className?: string;
}

export const StatCard = forwardRef<HTMLDivElement, StatCardProps>(
  (
    {
      title,
      value,
      unit,
      icon,
      trend,
      loading = false,
      details,
      className = '',
    },
    ref
  ) => {
    const [displayValue, setDisplayValue] = useState<string | number>(0);
    const [isAnimating, setIsAnimating] = useState(false);

    // Animate number changes
    useEffect(() => {
      if (loading) return;
      
      const numValue = typeof value === 'number' ? value : parseFloat(value.toString().replace(/,/g, ''));
      if (isNaN(numValue)) {
        setDisplayValue(value);
        return;
      }

      setIsAnimating(true);
      const duration = 500;
      const startTime = Date.now();
      const startValue = typeof displayValue === 'number' ? displayValue : 0;

      const animate = () => {
        const elapsed = Date.now() - startTime;
        const progress = Math.min(elapsed / duration, 1);
        const easeOut = 1 - Math.pow(1 - progress, 3);
        const current = startValue + (numValue - startValue) * easeOut;
        
        setDisplayValue(Math.round(current));
        
        if (progress < 1) {
          requestAnimationFrame(animate);
        } else {
          setDisplayValue(numValue);
          setIsAnimating(false);
        }
      };

      requestAnimationFrame(animate);
    }, [value, loading]);

    const classNames = [
      'ccr-stat-card',
      isAnimating && 'ccr-stat-card--animating',
      className,
    ]
      .filter(Boolean)
      .join(' ');

    if (loading) {
      return (
        <div ref={ref} className={classNames}>
          <div className="ccr-stat-card__header">
            <Skeleton variant="text" width="50%" height={16} />
            <Skeleton variant="circular" width={28} height={28} />
          </div>
          <div className="ccr-stat-card__value-container">
            <Skeleton variant="text" width="70%" height={36} />
          </div>
          {details && (
            <div className="ccr-stat-card__details">
              <Skeleton variant="text" width="100%" height={40} />
            </div>
          )}
        </div>
      );
    }

    const formattedValue = typeof displayValue === 'number' 
      ? displayValue.toLocaleString() 
      : displayValue;

    return (
      <div ref={ref} className={classNames}>
        <div className="ccr-stat-card__header">
          <h3 className="ccr-stat-card__title">{title}</h3>
          {icon && <span className="ccr-stat-card__icon">{icon}</span>}
        </div>
        
        <div className="ccr-stat-card__value-container">
          <span className="ccr-stat-card__value">{formattedValue}</span>
          {unit && <span className="ccr-stat-card__unit">{unit}</span>}
          {trend && (
            <span className={`ccr-stat-card__trend ccr-stat-card__trend--${trend.direction}`}>
              {trend.direction === 'up' ? '↑' : '↓'} {Math.abs(trend.value)}%
            </span>
          )}
        </div>

        {details && details.length > 0 && (
          <div className="ccr-stat-card__details">
            {details.map((detail, index) => (
              <div key={index} className="ccr-stat-card__detail-item">
                <span className="ccr-stat-card__detail-label">{detail.label}</span>
                <span className="ccr-stat-card__detail-value">{detail.value}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }
);

StatCard.displayName = 'StatCard';
