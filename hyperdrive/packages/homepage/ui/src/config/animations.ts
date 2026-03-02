import type { Transition, Variants } from 'framer-motion';

// Spring presets for consistent physics across the app
export const springs = {
  // Snappy - for quick micro-interactions (button taps, status changes)
  snappy: { type: 'spring', stiffness: 400, damping: 30 } as Transition,

  // Bouncy - for playful elements (reactions, emojis)
  bouncy: { type: 'spring', stiffness: 300, damping: 20, mass: 0.8 } as Transition,

  // Smooth - for larger movements (message entrance, panels)
  smooth: { type: 'spring', stiffness: 200, damping: 25 } as Transition,

  // Gentle - for subtle animations (fades, hover states)
  gentle: { type: 'spring', stiffness: 150, damping: 20 } as Transition,
};

// Stagger configurations for list animations
export const stagger = {
  fast: 0.02,    // For chat load (many items)
  normal: 0.04,  // For reactions appearing
  slow: 0.06,    // For emphasis
};

// Message entrance variants - quick subtle slide
export const messageVariants: Variants = {
  hidden: { opacity: 0, y: 5, scale: 0.98 },
  visible: {
    opacity: 1,
    y: 0,
    scale: 1,
    transition: { type: 'spring', stiffness: 400, damping: 30 }
  },
};

// Container variants for staggered message list
export const listContainerVariants: Variants = {
  hidden: { opacity: 1 },
  visible: {
    opacity: 1,
    transition: {
      staggerChildren: 0.008,
      delayChildren: 0.02,
    },
  },
};

// Send button animation states
export const sendButtonVariants: Variants = {
  idle: { scale: 1 },
  hover: { scale: 1.08 },
  tap: { scale: 0.9 },
  sending: {
    scale: [1, 0.85, 1.15, 1],
    transition: { duration: 0.35, times: [0, 0.25, 0.6, 1] }
  },
};

// Status indicator transitions (... → ✓ → ✓✓)
export const statusVariants: Variants = {
  initial: { opacity: 0, scale: 0.3 },
  animate: { opacity: 1, scale: 1 },
  exit: { opacity: 0, scale: 0.3 },
};

// Reaction emoji pop-in animation
export const reactionVariants: Variants = {
  initial: { opacity: 0, scale: 0, y: 8 },
  animate: {
    opacity: 1,
    scale: 1,
    y: 0,
  },
  exit: { opacity: 0, scale: 0, y: -8 },
  tap: { scale: 0.85 },
};

// Reply/Edit preview panel slide animation
export const previewVariants: Variants = {
  initial: { height: 0, opacity: 0 },
  animate: {
    height: 'auto',
    opacity: 1,
    transition: { type: 'spring', stiffness: 300, damping: 28 }
  },
  exit: {
    height: 0,
    opacity: 0,
    transition: { duration: 0.15 }
  },
};

// Scroll-to-bottom button appearance
export const scrollButtonVariants: Variants = {
  initial: { opacity: 0, y: 20, scale: 0.8 },
  animate: { opacity: 1, y: 0, scale: 1 },
  exit: { opacity: 0, y: 20, scale: 0.8 },
  hover: { scale: 1.1 },
  tap: { scale: 0.92 },
};

// Date separator fade in
export const dateSeparatorVariants: Variants = {
  hidden: { opacity: 0, scale: 0.9 },
  visible: {
    opacity: 1,
    scale: 1,
    transition: { type: 'spring', stiffness: 200, damping: 20 }
  },
};
