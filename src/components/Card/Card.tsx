import { forwardRef } from 'react';
import {
  Card as MuiCard,
  CardHeader,
  CardContent,
  Skeleton,
} from '@mui/material';

export interface CardProps {
  title?: string;
  subtitle?: string;
  icon?: React.ReactNode;
  actions?: React.ReactNode;
  loading?: boolean;
  hoverable?: boolean;
  className?: string;
  children: React.ReactNode;
}

export const Card = forwardRef<HTMLDivElement, CardProps>(
  (
    {
      title,
      subtitle,
      icon,
      actions,
      loading = false,
      children,
      className,
    },
    ref
  ) => {
    if (loading) {
      return (
        <MuiCard ref={ref} className={className}>
          {(title || icon) && (
            <CardHeader
              avatar={icon && <Skeleton variant="circular" width={40} height={40} />}
              title={<Skeleton variant="text" width="60%" />}
              subheader={subtitle && <Skeleton variant="text" width="40%" />}
            />
          )}
          <CardContent>
            <Skeleton variant="rectangular" width="100%" height={120} />
          </CardContent>
        </MuiCard>
      );
    }

    return (
      <MuiCard ref={ref} className={className}>
        {(title || icon || actions) && (
          <CardHeader
            avatar={icon}
            title={title}
            subheader={subtitle}
            action={actions}
          />
        )}
        <CardContent>{children}</CardContent>
      </MuiCard>
    );
  }
);

Card.displayName = 'Card';

export default Card;
