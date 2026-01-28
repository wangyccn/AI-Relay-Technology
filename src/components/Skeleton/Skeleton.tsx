import { forwardRef } from 'react';
import { Skeleton as MuiSkeleton, SkeletonProps as MuiSkeletonProps } from '@mui/material';

export interface SkeletonProps extends Omit<MuiSkeletonProps, 'variant'> {
  variant?: 'text' | 'circular' | 'rectangular' | 'rounded';
  width?: string | number;
  height?: string | number;
  count?: number;
}

export const Skeleton = forwardRef<HTMLSpanElement, SkeletonProps>(
  ({ variant = 'text', width, height, count = 1, sx, ...props }, ref) => {
    // Material Design 3 风格样式
    const customSx = {
      ...sx,
    };

    // 如果有多个骨架屏
    if (count > 1) {
      return (
        <>
          {Array.from({ length: count }).map((_, index) => (
            <MuiSkeleton
              key={index}
              variant={variant}
              width={width}
              height={height}
              sx={customSx}
              {...props}
            />
          ))}
        </>
      );
    }

    return (
      <MuiSkeleton
        ref={ref}
        variant={variant}
        width={width}
        height={height}
        sx={customSx}
        {...props}
      />
    );
  }
);

Skeleton.displayName = 'Skeleton';

export default Skeleton;
