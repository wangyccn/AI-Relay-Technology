import { useState, useMemo } from 'react';
import {
  Table as MuiTable,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TablePagination,
  TableRow,
  TableSortLabel,
  Checkbox,
  Paper,
  SxProps,
  Theme,
} from '@mui/material';
import {
  ArrowUpward as ArrowUpwardIcon,
  ArrowDownward as ArrowDownwardIcon,
} from '@mui/icons-material';
import { Skeleton } from '@mui/material';

export interface Column<T> {
  key: keyof T | string;
  title: string;
  width?: string | number;
  align?: 'left' | 'center' | 'right';
  sortable?: boolean;
  render?: (value: any, record: T, index: number) => React.ReactNode;
}

export interface TableProps<T> {
  columns: Column<T>[];
  data: T[];
  loading?: boolean;
  emptyText?: string;
  emptyIcon?: React.ReactNode;
  rowKey?: keyof T | ((record: T) => string);
  selectable?: boolean;
  selectedKeys?: string[];
  onSelectionChange?: (keys: string[]) => void;
  stickyHeader?: boolean;
  onSort?: (key: string, direction: 'asc' | 'desc') => void;
  pagination?: boolean;
  rowsPerPage?: number;
  sx?: SxProps<Theme>;
}

export function Table<T extends Record<string, any>>({
  columns,
  data,
  loading = false,
  emptyText = '暂无数据',
  emptyIcon,
  rowKey = 'id',
  selectable = false,
  selectedKeys = [],
  onSelectionChange,
  stickyHeader = true,
  onSort,
  pagination = true,
  rowsPerPage = 10,
  sx,
}: TableProps<T>) {
  const [page, setPage] = useState(0);
  const [sortKey, setSortKey] = useState<string | null>(null);
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('asc');

  const handleChangePage = (_event: unknown, newPage: number) => {
    setPage(newPage);
  };

  const handleChangeRowsPerPage = (_event: React.ChangeEvent<HTMLInputElement>) => {
    setPage(0);
  };

  const getRowKey = (record: T, index: number): string => {
    if (typeof rowKey === 'function') {
      return rowKey(record);
    }
    return String(record[rowKey] ?? index);
  };

  const getValue = (record: T, key: string): any => {
    const keys = key.split('.');
    let value: any = record;
    for (const k of keys) {
      value = value?.[k];
    }
    return value;
  };

  const handleSort = (key: string) => {
    const newDirection = sortKey === key && sortDirection === 'asc' ? 'desc' : 'asc';
    setSortKey(key);
    setSortDirection(newDirection);
    onSort?.(key, newDirection);
  };

  const handleSelectAll = () => {
    if (!onSelectionChange) return;

    const allKeys = data.map((record, index) => getRowKey(record, index));
    const allSelected = allKeys.every((key) => selectedKeys.includes(key));

    if (allSelected) {
      onSelectionChange([]);
    } else {
      onSelectionChange(allKeys);
    }
  };

  const handleSelectRow = (key: string) => {
    if (!onSelectionChange) return;

    if (selectedKeys.includes(key)) {
      onSelectionChange(selectedKeys.filter((k) => k !== key));
    } else {
      onSelectionChange([...selectedKeys, key]);
    }
  };

  const allSelected = useMemo(() => {
    if (data.length === 0) return false;
    return data.every((record, index) =>
      selectedKeys.includes(getRowKey(record, index))
    );
  }, [data, selectedKeys]);

  const someSelected = useMemo(() => {
    return data.some((record, index) =>
      selectedKeys.includes(getRowKey(record, index))
    );
  }, [data, selectedKeys]);

  // 分页数据
  const paginatedData = useMemo(() => {
    if (!pagination) return data;
    return data.slice(page * rowsPerPage, page * rowsPerPage + rowsPerPage);
  }, [data, page, rowsPerPage, pagination]);

  // Material Design 3 风格样式
  const customSx: SxProps<Theme> = {
    '& .MuiTableCell-root': {
      borderColor: 'divider',
    },
    '& .MuiTableHead-root .MuiTableCell-root': {
      bgcolor: 'action.hover',
      fontWeight: 600,
      fontSize: '0.75rem',
      textTransform: 'uppercase',
      letterSpacing: '0.05em',
    },
    '& .MuiTableRow-root:hover': {
      bgcolor: 'action.hover',
    },
    ...sx,
  };

  if (loading) {
    return (
      <TableContainer component={Paper} sx={customSx}>
        <MuiTable stickyHeader={stickyHeader}>
          <TableHead>
            <TableRow>
              {selectable && <TableCell padding="checkbox" />}
              {columns.map((column) => (
                <TableCell
                  key={String(column.key)}
                  align={column.align || 'left'}
                  sx={{ width: column.width }}
                >
                  <Skeleton variant="text" width="80%" />
                </TableCell>
              ))}
            </TableRow>
          </TableHead>
          <TableBody>
            {Array.from({ length: rowsPerPage }).map((_, index) => (
              <TableRow key={index}>
                {selectable && (
                  <TableCell padding="checkbox">
                    <Skeleton variant="rectangular" width={20} height={20} />
                  </TableCell>
                )}
                {columns.map((column) => (
                  <TableCell key={String(column.key)}>
                    <Skeleton variant="text" />
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </MuiTable>
      </TableContainer>
    );
  }

  return (
    <TableContainer component={Paper} sx={customSx}>
      <MuiTable stickyHeader={stickyHeader}>
        <TableHead>
          <TableRow>
            {selectable && (
              <TableCell padding="checkbox">
                <Checkbox
                  indeterminate={someSelected && !allSelected}
                  checked={allSelected}
                  onChange={handleSelectAll}
                  sx={{ borderRadius: 1 }}
                />
              </TableCell>
            )}
            {columns.map((column) => (
              <TableCell
                key={String(column.key)}
                align={column.align || 'left'}
                sx={{ width: column.width }}
              >
                {column.sortable ? (
                  <TableSortLabel
                    active={sortKey === String(column.key)}
                    direction={sortDirection}
                    onClick={() => handleSort(String(column.key))}
                    IconComponent={
                      sortKey === String(column.key) && sortDirection === 'desc'
                        ? ArrowDownwardIcon
                        : ArrowUpwardIcon
                    }
                  >
                    {column.title}
                  </TableSortLabel>
                ) : (
                  column.title
                )}
              </TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {paginatedData.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={columns.length + (selectable ? 1 : 0)}
                align="center"
                sx={{ py: 8 }}
              >
                <div
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    gap: 2,
                  }}
                >
                  {emptyIcon && (
                    <div style={{ fontSize: 48, opacity: 0.5 }}>{emptyIcon}</div>
                  )}
                  <div style={{ color: 'text.secondary' }}>{emptyText}</div>
                </div>
              </TableCell>
            </TableRow>
          ) : (
            paginatedData.map((record, index) => {
              const key = getRowKey(record, index);
              const isSelected = selectedKeys.includes(key);

              return (
                <TableRow
                  key={key}
                  hover
                  selected={isSelected}
                  sx={{ cursor: onSelectionChange ? 'pointer' : 'default' }}
                  onClick={() =>
                    selectable && onSelectionChange && handleSelectRow(key)
                  }
                >
                  {selectable && (
                    <TableCell padding="checkbox">
                      <Checkbox
                        checked={isSelected}
                        sx={{ borderRadius: 1 }}
                        onClick={(e) => {
                          e.stopPropagation();
                          handleSelectRow(key);
                        }}
                      />
                    </TableCell>
                  )}
                  {columns.map((column) => {
                    const value = getValue(record, String(column.key));
                    const content = column.render
                      ? column.render(value, record, index)
                      : value;

                    return (
                      <TableCell
                        key={String(column.key)}
                        align={column.align || 'left'}
                      >
                        {content as React.ReactNode}
                      </TableCell>
                    );
                  })}
                </TableRow>
              );
            })
          )}
        </TableBody>
      </MuiTable>
      {pagination && data.length > 0 && (
        <TablePagination
          rowsPerPageOptions={[5, 10, 25, 50]}
          component="div"
          count={data.length}
          rowsPerPage={rowsPerPage}
          page={page}
          onPageChange={handleChangePage}
          onRowsPerPageChange={handleChangeRowsPerPage}
          labelRowsPerPage="每页行数:"
          labelDisplayedRows={({ from, to, count }) =>
            `${from}-${to} 共 ${count !== -1 ? count : `超过 ${to}`} 条`
          }
        />
      )}
    </TableContainer>
  );
}

export default Table;
