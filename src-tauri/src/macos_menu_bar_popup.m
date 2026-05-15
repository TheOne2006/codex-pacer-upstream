#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <float.h>
#import <math.h>
#import <stdbool.h>

typedef char *(*CodexPacerSnapshotProvider)(bool forceRefresh);
typedef void (*CodexPacerSnapshotFree)(char *value);
typedef void (*CodexPacerActionHandler)(const char *action);

static const CGFloat CPPanelWidth = 420.0;
static const CGFloat CPPanelHeight = 720.0;
static const CGFloat CPPanelAnimationOffset = 10.0;
static const CGFloat CPPanelAnimationInset = 6.0;
static const NSTimeInterval CPPanelOpenAnimationDuration = 0.18;
static const NSTimeInterval CPPanelCloseAnimationDuration = 0.14;

static NSColor *CPColor(CGFloat r, CGFloat g, CGFloat b, CGFloat a) {
    return [NSColor colorWithCalibratedRed:r green:g blue:b alpha:a];
}

static NSRect CPPanelAnimationFrame(NSRect frame) {
    NSRect animatedFrame = NSInsetRect(frame, CPPanelAnimationInset, CPPanelAnimationInset);
    animatedFrame.origin.y += CPPanelAnimationOffset;
    return animatedFrame;
}

static NSDictionary *CPAsDictionary(id value) {
    return [value isKindOfClass:NSDictionary.class] ? (NSDictionary *)value : @{};
}

static NSArray *CPAsArray(id value) {
    return [value isKindOfClass:NSArray.class] ? (NSArray *)value : @[];
}

static NSNumber *CPNumber(NSDictionary *dict, NSString *key) {
    id value = dict[key];
    return [value isKindOfClass:NSNumber.class] ? (NSNumber *)value : nil;
}

static NSString *CPString(NSDictionary *dict, NSString *key) {
    id value = dict[key];
    return [value isKindOfClass:NSString.class] ? (NSString *)value : nil;
}

static BOOL CPIsChineseLanguage(NSString *language) {
    return ![language isEqualToString:@"en"];
}

static BOOL CPIsChineseSnapshot(NSDictionary *snapshot) {
    return CPIsChineseLanguage(CPString(snapshot, @"displayLanguage"));
}

static NSString *CPT(BOOL zh, NSString *zhText, NSString *enText) {
    return zh ? zhText : enText;
}

static NSString *CPBucketLabel(NSString *bucket, BOOL zh) {
    if ([bucket isEqualToString:@"day"]) return CPT(zh, @"今日", @"Today");
    if ([bucket isEqualToString:@"five_hour"]) return CPT(zh, @"5小时", @"5h");
    if ([bucket isEqualToString:@"seven_day"]) return CPT(zh, @"7天", @"7d");
    if ([bucket isEqualToString:@"week"]) return CPT(zh, @"本周", @"This week");
    if ([bucket isEqualToString:@"month"]) return CPT(zh, @"本月", @"This month");
    if ([bucket isEqualToString:@"year"]) return CPT(zh, @"本年", @"This year");
    if ([bucket isEqualToString:@"custom"]) return CPT(zh, @"自定义", @"Custom");
    if ([bucket isEqualToString:@"total"]) return CPT(zh, @"总计", @"Total");
    return bucket.length > 0 ? bucket : CPT(zh, @"本月", @"This month");
}

static NSString *CPUSD(double value) {
    NSNumberFormatter *formatter = [[NSNumberFormatter alloc] init];
    formatter.numberStyle = NSNumberFormatterCurrencyStyle;
    formatter.currencyCode = @"USD";
    formatter.maximumFractionDigits = value >= 100.0 ? 0 : 2;
    return [formatter stringFromNumber:@(value)] ?: [NSString stringWithFormat:@"$%.2f", value];
}

static NSString *CPCompactUnit(double value, double divisor, NSString *unit) {
    double scaled = value / divisor;
    double rounded = round(scaled * 10.0) / 10.0;
    if (fabs(rounded - round(rounded)) < 0.05) {
        return [NSString stringWithFormat:@"%.0f%@", rounded, unit];
    }
    return [NSString stringWithFormat:@"%.1f%@", rounded, unit];
}

static NSString *CPCompactInteger(long long value, BOOL zh) {
    double absValue = fabs((double)value);
    if (zh) {
        if (absValue >= 100000000.0) return CPCompactUnit((double)value, 100000000.0, @"亿");
        if (absValue >= 100000.0) return CPCompactUnit((double)value, 10000.0, @"万");
    } else {
        if (absValue >= 1000000.0) return CPCompactUnit((double)value, 1000000.0, @"M");
        if (absValue >= 100000.0) return CPCompactUnit((double)value, 1000.0, @"K");
    }
    NSNumberFormatter *formatter = [[NSNumberFormatter alloc] init];
    formatter.numberStyle = NSNumberFormatterDecimalStyle;
    return [formatter stringFromNumber:@(value)] ?: [NSString stringWithFormat:@"%lld", value];
}

static NSString *CPPercentRatio(double value) {
    return [NSString stringWithFormat:@"%.0f%%", value * 100.0];
}

static NSDate *CPDateFromString(NSString *value) {
    if (value.length == 0) return nil;
    NSISO8601DateFormatter *formatter = [[NSISO8601DateFormatter alloc] init];
    formatter.formatOptions = NSISO8601DateFormatWithInternetDateTime | NSISO8601DateFormatWithFractionalSeconds;
    NSDate *date = [formatter dateFromString:value];
    if (date) return date;
    formatter.formatOptions = NSISO8601DateFormatWithInternetDateTime;
    return [formatter dateFromString:value];
}

static NSTimeInterval CPTimeFromString(NSString *value) {
    NSDate *date = CPDateFromString(value);
    return date ? date.timeIntervalSince1970 : NAN;
}

static BOOL CPIsFiniteTime(NSTimeInterval value) {
    return isfinite(value);
}

static NSTimeInterval CPClampTime(NSTimeInterval value, NSTimeInterval min, NSTimeInterval max) {
    return fmax(min, fmin(max, value));
}

static NSTimeInterval CPRoundToMinute(NSTimeInterval value) {
    return round(value / 60.0) * 60.0;
}

static NSString *CPShortDateTime(NSString *value, BOOL zh) {
    NSDate *date = CPDateFromString(value);
    if (!date) return CPT(zh, @"无数据", @"No data");
    NSDateFormatter *formatter = [[NSDateFormatter alloc] init];
    formatter.locale = [NSLocale localeWithLocaleIdentifier:zh ? @"zh_CN" : @"en_US_POSIX"];
    formatter.dateFormat = zh ? @"M月d日 HH:mm" : @"MMM d HH:mm";
    return [formatter stringFromDate:date];
}

static NSString *CPRelativeTime(NSString *value, BOOL zh) {
    NSDate *date = CPDateFromString(value);
    if (!date) return CPT(zh, @"无数据", @"No data");
    NSTimeInterval seconds = -[date timeIntervalSinceNow];
    if (seconds < 60) return CPT(zh, @"刚刚", @"just now");
    if (seconds < 3600) return zh
        ? [NSString stringWithFormat:@"%.0f 分钟前", floor(seconds / 60.0)]
        : [NSString stringWithFormat:@"%.0fm ago", floor(seconds / 60.0)];
    if (seconds < 86400) return zh
        ? [NSString stringWithFormat:@"%.0f 小时前", floor(seconds / 3600.0)]
        : [NSString stringWithFormat:@"%.0fh ago", floor(seconds / 3600.0)];
    return zh
        ? [NSString stringWithFormat:@"%.0f 天前", floor(seconds / 86400.0)]
        : [NSString stringWithFormat:@"%.0fd ago", floor(seconds / 86400.0)];
}

static NSTextField *CPLabel(NSString *text, NSFont *font, NSColor *color, NSInteger lines) {
    NSTextField *label = [NSTextField labelWithString:text ?: @""];
    label.font = font;
    label.textColor = color;
    label.maximumNumberOfLines = lines;
    label.lineBreakMode = NSLineBreakByTruncatingTail;
    label.translatesAutoresizingMaskIntoConstraints = NO;
    return label;
}

static NSView *CPCardView(CGFloat radius) {
    NSView *view = [[NSView alloc] initWithFrame:NSZeroRect];
    view.translatesAutoresizingMaskIntoConstraints = NO;
    view.wantsLayer = YES;
    view.layer.cornerRadius = radius;
    view.layer.backgroundColor = CPColor(1.0, 1.0, 1.0, 0.055).CGColor;
    view.layer.borderColor = CPColor(1.0, 1.0, 1.0, 0.10).CGColor;
    view.layer.borderWidth = 1.0;
    return view;
}

static NSString *CPPlainStatusTitle(NSString *apiValue, NSString *liveLabel, NSString *liveValue) {
    NSMutableArray<NSString *> *segments = [NSMutableArray array];
    if (apiValue.length > 0) [segments addObject:apiValue];
    if (liveValue.length > 0) [segments addObject:liveValue];
    if (segments.count == 0 && liveLabel.length > 0) [segments addObject:liveLabel];
    return [segments componentsJoinedByString:@" "];
}

static BOOL CPIsDarkAppearance(void) {
    if (@available(macOS 10.14, *)) {
    NSAppearance *appearance = NSApp.effectiveAppearance;
    if (!appearance) return NO;
    NSString *match = [appearance bestMatchFromAppearancesWithNames:@[
        NSAppearanceNameAqua,
        NSAppearanceNameDarkAqua,
    ]];
    return [match isEqualToString:NSAppearanceNameDarkAqua];
    }
    return NO;
}

static NSColor *CPStatsMiniForegroundColor(void) {
    return CPIsDarkAppearance() ? NSColor.whiteColor : NSColor.textColor;
}

static NSDictionary *CPStatusLabelAttributes(void) {
    return @{
        NSFontAttributeName: [NSFont systemFontOfSize:7.0 weight:NSFontWeightLight],
        NSForegroundColorAttributeName: CPStatsMiniForegroundColor(),
    };
}

static NSDictionary *CPStatusValueAttributes(void) {
    return @{
        NSFontAttributeName: [NSFont systemFontOfSize:12.0 weight:NSFontWeightRegular],
        NSForegroundColorAttributeName: CPStatsMiniForegroundColor(),
    };
}

static NSDictionary *CPStatusSingleLineAttributes(void) {
    return @{
        NSFontAttributeName: [NSFont monospacedDigitSystemFontOfSize:12.0 weight:NSFontWeightMedium],
        NSForegroundColorAttributeName: NSColor.labelColor,
    };
}

static CGFloat CPStatusTextWidth(NSString *text, NSDictionary *attributes) {
    return ceil([(text ?: @"") sizeWithAttributes:attributes].width);
}

@protocol CPStatusItemViewDelegate <NSObject>
- (void)statusItemViewLeftClicked:(id)sender;
- (void)statusItemViewRightClicked:(id)sender;
@end

@interface CPStatusItemView : NSView
@property(nonatomic, weak) id<CPStatusItemViewDelegate> delegate;
@property(nonatomic, copy) NSString *apiValueTitle;
@property(nonatomic, copy) NSString *liveMetricLabel;
@property(nonatomic, copy) NSString *liveMetricValue;
@property(nonatomic) BOOL showLogo;
@property(nonatomic, strong) NSImage *icon;
- (CGFloat)preferredWidth;
@end

@implementation CPStatusItemView
- (BOOL)isFlipped { return YES; }
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
- (NSView *)hitTest:(NSPoint)point { return nil; }

- (CGFloat)preferredWidth {
    NSString *liveValue = self.liveMetricValue ?: @"";
    if (liveValue.length > 0) {
        NSString *top = self.liveMetricLabel.length > 0 ? self.liveMetricLabel : self.apiValueTitle;
        CGFloat textWidth = MAX(CPStatusTextWidth(top, CPStatusLabelAttributes()),
                                CPStatusTextWidth(liveValue, CPStatusValueAttributes()));
        return MAX(34.0, textWidth + 8.0);
    }

    NSString *title = CPPlainStatusTitle(self.apiValueTitle ?: @"", self.liveMetricLabel ?: @"", liveValue);
    CGFloat width = title.length > 0 ? CPStatusTextWidth(title, CPStatusSingleLineAttributes()) + 8.0 : 0.0;
    if (self.showLogo) width += 22.0;
    return MAX(self.showLogo ? 24.0 : 18.0, width);
}

- (void)drawRect:(NSRect)dirtyRect {
    [super drawRect:dirtyRect];
    [[NSColor clearColor] setFill];
    NSRectFill(self.bounds);

    NSString *liveValue = self.liveMetricValue ?: @"";
    if (liveValue.length > 0) {
        NSString *top = self.liveMetricLabel.length > 0 ? self.liveMetricLabel : self.apiValueTitle;
        CGFloat width = NSWidth(self.bounds) - 8.0;
        [top drawWithRect:NSMakeRect(4.0, 1.5, width, 7.0)
                  options:NSStringDrawingUsesLineFragmentOrigin
               attributes:CPStatusLabelAttributes()];
        [liveValue drawWithRect:NSMakeRect(4.0, 6.0, width, 13.0)
                        options:NSStringDrawingUsesLineFragmentOrigin
                     attributes:CPStatusValueAttributes()];
        return;
    }

    CGFloat x = 4.0;
    if (self.showLogo && self.icon) {
        NSRect iconRect = NSMakeRect(x, floor((NSHeight(self.bounds) - 18.0) / 2.0), 18.0, 18.0);
        [self.icon drawInRect:iconRect];
        x += 22.0;
    }

    NSString *title = CPPlainStatusTitle(self.apiValueTitle ?: @"", self.liveMetricLabel ?: @"", liveValue);
    if (title.length == 0) return;
    NSDictionary *attrs = CPStatusSingleLineAttributes();
    CGFloat y = floor((NSHeight(self.bounds) - [title sizeWithAttributes:attrs].height) / 2.0);
    [title drawAtPoint:NSMakePoint(x, y) withAttributes:attrs];
}

- (void)mouseUp:(NSEvent *)event {
    if ((event.modifierFlags & NSEventModifierFlagControl) == NSEventModifierFlagControl) {
        [self.delegate statusItemViewRightClicked:self];
    } else {
        [self.delegate statusItemViewLeftClicked:self];
    }
}

- (void)rightMouseUp:(NSEvent *)event {
    [self.delegate statusItemViewRightClicked:self];
}
@end

@interface CPQuotaRingView : NSView
@property(nonatomic) CGFloat remainingPercent;
@property(nonatomic) CGFloat timePercent;
@property(nonatomic, strong) NSColor *accentColor;
@property(nonatomic, copy) NSString *title;
@end

@implementation CPQuotaRingView
- (BOOL)isFlipped { return YES; }
- (void)drawRect:(NSRect)dirtyRect {
    [super drawRect:dirtyRect];
    CGFloat side = MIN(NSWidth(self.bounds), NSHeight(self.bounds));
    NSPoint center = NSMakePoint(NSMidX(self.bounds), NSMidY(self.bounds) - 3.0);
    CGFloat radius = side * 0.34;
    NSColor *track = CPColor(1.0, 1.0, 1.0, 0.12);
    NSColor *accent = self.accentColor ?: CPColor(1.0, 0.70, 0.42, 1.0);

    NSBezierPath *outerTrack = [NSBezierPath bezierPath];
    [outerTrack appendBezierPathWithArcWithCenter:center radius:radius startAngle:-90 endAngle:270 clockwise:NO];
    outerTrack.lineWidth = 10.0;
    [track setStroke];
    [outerTrack stroke];

    NSBezierPath *outer = [NSBezierPath bezierPath];
    CGFloat sweep = MAX(0.0, MIN(100.0, self.remainingPercent)) / 100.0 * 360.0;
    [outer appendBezierPathWithArcWithCenter:center radius:radius startAngle:-90 endAngle:-90 + sweep clockwise:NO];
    outer.lineWidth = 10.0;
    outer.lineCapStyle = NSLineCapStyleRound;
    [accent setStroke];
    [outer stroke];

    NSBezierPath *innerTrack = [NSBezierPath bezierPath];
    [innerTrack appendBezierPathWithArcWithCenter:center radius:radius - 14.0 startAngle:-90 endAngle:270 clockwise:NO];
    innerTrack.lineWidth = 5.0;
    [CPColor(1.0, 1.0, 1.0, 0.14) setStroke];
    [innerTrack stroke];

    NSBezierPath *inner = [NSBezierPath bezierPath];
    CGFloat timeSweep = MAX(0.0, MIN(100.0, self.timePercent)) / 100.0 * 360.0;
    [inner appendBezierPathWithArcWithCenter:center radius:radius - 14.0 startAngle:-90 endAngle:-90 + timeSweep clockwise:NO];
    inner.lineWidth = 5.0;
    inner.lineCapStyle = NSLineCapStyleRound;
    [[accent colorWithAlphaComponent:0.62] setStroke];
    [inner stroke];

    NSString *percent = [NSString stringWithFormat:@"%.0f%%", self.remainingPercent];
    NSDictionary *percentAttrs = @{
        NSFontAttributeName: [NSFont monospacedDigitSystemFontOfSize:24 weight:NSFontWeightBold],
        NSForegroundColorAttributeName: CPColor(0.98, 0.93, 0.84, 1.0),
    };
    NSSize percentSize = [percent sizeWithAttributes:percentAttrs];
    [percent drawAtPoint:NSMakePoint(center.x - percentSize.width / 2.0, center.y - 17.0) withAttributes:percentAttrs];

    NSDictionary *titleAttrs = @{
        NSFontAttributeName: [NSFont systemFontOfSize:11 weight:NSFontWeightSemibold],
        NSForegroundColorAttributeName: CPColor(0.73, 0.69, 0.62, 1.0),
    };
    NSSize titleSize = [self.title ?: @"Quota" sizeWithAttributes:titleAttrs];
    [(self.title ?: @"Quota") drawAtPoint:NSMakePoint(center.x - titleSize.width / 2.0, center.y + 11.0) withAttributes:titleAttrs];
}
@end

@interface CPChartLegendMarkerView : NSView
@property(nonatomic, copy) NSString *kind;
@end

@implementation CPChartLegendMarkerView
- (BOOL)isFlipped { return YES; }
- (void)drawRect:(NSRect)dirtyRect {
    [super drawRect:dirtyRect];
    if ([self.kind isEqualToString:@"current"]) {
        NSRect dot = NSInsetRect(self.bounds, 5.0, 1.0);
        NSBezierPath *path = [NSBezierPath bezierPathWithOvalInRect:dot];
        [CPColor(1.0, 0.97, 0.91, 1.0) setFill];
        [path fill];
        [CPColor(0.46, 0.77, 1.0, 1.0) setStroke];
        path.lineWidth = 2.0;
        [path stroke];
        return;
    }

    NSBezierPath *path = [NSBezierPath bezierPath];
    CGFloat y = NSMidY(self.bounds);
    [path moveToPoint:NSMakePoint(NSMinX(self.bounds), y)];
    [path lineToPoint:NSMakePoint(NSMaxX(self.bounds), y)];
    path.lineCapStyle = NSLineCapStyleRound;
    path.lineWidth = [self.kind isEqualToString:@"reference"] ? 2.0 : 3.0;
    if ([self.kind isEqualToString:@"reference"]) {
        CGFloat pattern[] = {5.0, 5.0};
        [path setLineDash:pattern count:2 phase:0.0];
        [CPColor(1.0, 0.86, 0.63, 0.78) setStroke];
    } else {
        [CPColor(0.46, 0.77, 1.0, 1.0) setStroke];
    }
    [path stroke];
}
@end

@interface CPTrendChartView : NSView
@property(nonatomic, strong) NSArray *points;
@property(nonatomic, strong) NSDictionary *quota;
@property(nonatomic, copy) NSString *fetchedAt;
@property(nonatomic, copy) NSString *emptyLabel;
@end

@implementation CPTrendChartView
- (BOOL)isFlipped { return YES; }

- (NSArray *)valuePointsWithCurrentTime:(NSTimeInterval *)currentOut
                            windowStart:(NSTimeInterval *)startOut
                              windowEnd:(NSTimeInterval *)endOut {
    static const NSTimeInterval sevenDays = 7.0 * 24.0 * 60.0 * 60.0;
    NSArray *rawPoints = self.points ?: @[];
    NSDictionary *quota = self.quota ?: @{};
    NSMutableArray<NSNumber *> *dataTimes = [NSMutableArray array];
    for (id rawPoint in rawPoints) {
        NSDictionary *point = CPAsDictionary(rawPoint);
        NSTimeInterval timestamp = CPTimeFromString(CPString(point, @"timestamp"));
        if (CPIsFiniteTime(timestamp)) [dataTimes addObject:@(timestamp)];
    }

    NSTimeInterval fetched = CPTimeFromString(self.fetchedAt);
    NSTimeInterval fallbackCurrent = CPIsFiniteTime(fetched) ? fetched : NAN;
    if (!CPIsFiniteTime(fallbackCurrent) && dataTimes.count > 0) {
        fallbackCurrent = dataTimes.lastObject.doubleValue;
    }
    if (!CPIsFiniteTime(fallbackCurrent)) {
        NSTimeInterval reset = CPTimeFromString(CPString(quota, @"resetsAt"));
        fallbackCurrent = CPIsFiniteTime(reset) ? reset - sevenDays : NSDate.date.timeIntervalSince1970;
    }

    NSTimeInterval fallbackStart = fallbackCurrent - sevenDays;
    if (dataTimes.count > 0) {
        fallbackStart = DBL_MAX;
        for (NSNumber *time in dataTimes) fallbackStart = fmin(fallbackStart, time.doubleValue);
    }
    NSTimeInterval windowStart = CPTimeFromString(CPString(quota, @"windowStart"));
    if (!CPIsFiniteTime(windowStart)) windowStart = fallbackStart;

    NSTimeInterval maxDataTime = windowStart;
    for (NSNumber *time in dataTimes) maxDataTime = fmax(maxDataTime, time.doubleValue);
    NSTimeInterval windowEnd = CPTimeFromString(CPString(quota, @"resetsAt"));
    if (!CPIsFiniteTime(windowEnd)) windowEnd = fmax(windowStart + sevenDays, maxDataTime);
    if (windowEnd <= windowStart) windowEnd = windowStart + sevenDays;

    NSTimeInterval currentTime = CPClampTime(fallbackCurrent, windowStart, windowEnd);
    NSMutableDictionary<NSNumber *, NSNumber *> *byMinute = [NSMutableDictionary dictionary];
    for (id rawPoint in rawPoints) {
        NSDictionary *point = CPAsDictionary(rawPoint);
        NSNumber *remaining = CPNumber(point, @"remainingPercent");
        NSTimeInterval timestamp = CPTimeFromString(CPString(point, @"timestamp"));
        if (!remaining || !CPIsFiniteTime(timestamp) || timestamp < windowStart || timestamp > windowEnd) continue;
        byMinute[@(CPRoundToMinute(timestamp))] = @(fmax(0.0, fmin(100.0, remaining.doubleValue)));
    }

    NSNumber *quotaRemaining = CPNumber(quota, @"remainingPercent");
    if (quotaRemaining) {
        byMinute[@(CPRoundToMinute(currentTime))] = @(fmax(0.0, fmin(100.0, quotaRemaining.doubleValue)));
    }

    NSArray<NSNumber *> *keys = [[byMinute allKeys] sortedArrayUsingSelector:@selector(compare:)];
    NSMutableArray<NSDictionary *> *points = [NSMutableArray array];
    if (keys.count > 0 && keys.firstObject.doubleValue > windowStart) {
        [points addObject:@{@"time": @(windowStart), @"value": @100.0}];
    }
    for (NSNumber *key in keys) {
        [points addObject:@{@"time": key, @"value": byMinute[key]}];
    }

    if (currentOut) *currentOut = CPRoundToMinute(currentTime);
    if (startOut) *startOut = windowStart;
    if (endOut) *endOut = windowEnd;
    return points;
}

- (NSPoint)plotPointForTime:(NSTimeInterval)time value:(double)value inRect:(NSRect)plot windowStart:(NSTimeInterval)windowStart windowEnd:(NSTimeInterval)windowEnd {
    double ratio = (time - windowStart) / fmax(1.0, windowEnd - windowStart);
    CGFloat x = NSMinX(plot) + CPClampTime(ratio, 0.0, 1.0) * NSWidth(plot);
    CGFloat y = NSMinY(plot) + (1.0 - fmax(0.0, fmin(100.0, value)) / 100.0) * NSHeight(plot);
    return NSMakePoint(x, y);
}

- (NSBezierPath *)smoothPathForPoints:(NSArray<NSValue *> *)plotPoints {
    NSBezierPath *path = [NSBezierPath bezierPath];
    if (plotPoints.count == 0) return path;
    NSPoint first = plotPoints.firstObject.pointValue;
    [path moveToPoint:first];
    for (NSUInteger index = 1; index < plotPoints.count; index += 1) {
        NSPoint previous = plotPoints[index - 1].pointValue;
        NSPoint current = plotPoints[index].pointValue;
        CGFloat midpointX = (previous.x + current.x) / 2.0;
        [path curveToPoint:current controlPoint1:NSMakePoint(midpointX, previous.y) controlPoint2:NSMakePoint(midpointX, current.y)];
    }
    return path;
}

- (void)drawRect:(NSRect)dirtyRect {
    [super drawRect:dirtyRect];
    [[NSColor clearColor] setFill];
    NSRectFill(self.bounds);

    NSRect plot = NSMakeRect(12.0, 10.0, NSWidth(self.bounds) - 24.0, NSHeight(self.bounds) - 24.0);
    NSTimeInterval currentTime = 0.0;
    NSTimeInterval windowStart = 0.0;
    NSTimeInterval windowEnd = 0.0;
    NSArray *valuePoints = [self valuePointsWithCurrentTime:&currentTime windowStart:&windowStart windowEnd:&windowEnd];
    if (valuePoints.count == 0) {
        NSDictionary *attrs = @{
            NSFontAttributeName: [NSFont systemFontOfSize:12 weight:NSFontWeightMedium],
            NSForegroundColorAttributeName: CPColor(0.73, 0.69, 0.62, 1.0),
        };
        [(self.emptyLabel ?: @"No seven-day usage trend") drawInRect:plot withAttributes:attrs];
        return;
    }

    [CPColor(1.0, 1.0, 1.0, 0.052) setStroke];
    for (NSInteger index = 0; index <= 7; index += 1) {
        CGFloat x = NSMinX(plot) + NSWidth(plot) * (CGFloat)index / 7.0;
        NSBezierPath *grid = [NSBezierPath bezierPath];
        [grid moveToPoint:NSMakePoint(x, NSMinY(plot))];
        [grid lineToPoint:NSMakePoint(x, NSMaxY(plot))];
        grid.lineWidth = 1.0;
        [grid stroke];
    }

    [CPColor(1.0, 1.0, 1.0, 0.075) setStroke];
    for (NSNumber *value in @[@25, @50, @75]) {
        CGFloat y = [self plotPointForTime:windowStart value:value.doubleValue inRect:plot windowStart:windowStart windowEnd:windowEnd].y;
        NSBezierPath *grid = [NSBezierPath bezierPath];
        [grid moveToPoint:NSMakePoint(NSMinX(plot), y)];
        [grid lineToPoint:NSMakePoint(NSMaxX(plot), y)];
        grid.lineWidth = 1.0;
        [grid stroke];
    }

    NSPoint referenceStart = [self plotPointForTime:windowStart value:100.0 inRect:plot windowStart:windowStart windowEnd:windowEnd];
    NSPoint referenceEnd = [self plotPointForTime:windowEnd value:0.0 inRect:plot windowStart:windowStart windowEnd:windowEnd];
    NSBezierPath *reference = [NSBezierPath bezierPath];
    [reference moveToPoint:referenceStart];
    [reference lineToPoint:referenceEnd];
    reference.lineWidth = 2.0;
    reference.lineCapStyle = NSLineCapStyleRound;
    CGFloat dash[] = {6.0, 7.0};
    [reference setLineDash:dash count:2 phase:0.0];
    [CPColor(1.0, 0.86, 0.63, 0.60) setStroke];
    [reference stroke];

    NSMutableArray<NSValue *> *plotPoints = [NSMutableArray array];
    NSValue *currentPlotPoint = nil;
    for (NSDictionary *point in valuePoints) {
        NSTimeInterval time = [point[@"time"] doubleValue];
        double value = [point[@"value"] doubleValue];
        NSPoint plotPoint = [self plotPointForTime:time value:value inRect:plot windowStart:windowStart windowEnd:windowEnd];
        NSValue *wrapped = [NSValue valueWithPoint:plotPoint];
        [plotPoints addObject:wrapped];
        if (fabs(time - currentTime) < 60.1) currentPlotPoint = wrapped;
    }
    if (!currentPlotPoint) currentPlotPoint = plotPoints.lastObject;

    if (plotPoints.count >= 2) {
        NSBezierPath *line = [self smoothPathForPoints:plotPoints];
        NSBezierPath *area = [line copy];
        NSPoint last = plotPoints.lastObject.pointValue;
        NSPoint first = plotPoints.firstObject.pointValue;
        [area lineToPoint:NSMakePoint(last.x, NSMaxY(plot))];
        [area lineToPoint:NSMakePoint(first.x, NSMaxY(plot))];
        [area closePath];
        [CPColor(0.46, 0.77, 1.0, 0.18) setFill];
        [area fill];

        line.lineWidth = 3.2;
        line.lineJoinStyle = NSLineJoinStyleRound;
        line.lineCapStyle = NSLineCapStyleRound;
        [CPColor(0.46, 0.77, 1.0, 1.0) setStroke];
        [line stroke];
    }

    if (currentPlotPoint) {
        NSPoint point = currentPlotPoint.pointValue;
        NSBezierPath *halo = [NSBezierPath bezierPathWithOvalInRect:NSMakeRect(point.x - 9.0, point.y - 9.0, 18.0, 18.0)];
        [CPColor(0.46, 0.77, 1.0, 0.18) setFill];
        [halo fill];
        [CPColor(1.0, 1.0, 1.0, 0.30) setStroke];
        halo.lineWidth = 1.0;
        [halo stroke];

        NSBezierPath *dot = [NSBezierPath bezierPathWithOvalInRect:NSMakeRect(point.x - 4.5, point.y - 4.5, 9.0, 9.0)];
        [CPColor(1.0, 0.97, 0.91, 1.0) setFill];
        [dot fill];
        [CPColor(0.46, 0.77, 1.0, 1.0) setStroke];
        dot.lineWidth = 2.2;
        [dot stroke];
    }
}
@end

@interface CPMenuBarPanel : NSPanel
@end

@implementation CPMenuBarPanel
- (BOOL)canBecomeKeyWindow { return YES; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

@interface CPNativeMenuBarController : NSObject <CPStatusItemViewDelegate>
@property(nonatomic, strong) NSStatusItem *statusItem;
@property(nonatomic, strong) CPStatusItemView *statusView;
@property(nonatomic, strong) CPMenuBarPanel *panel;
@property(nonatomic, strong) id localEventMonitor;
@property(nonatomic, strong) id globalEventMonitor;
@property(nonatomic) CodexPacerSnapshotProvider snapshotProvider;
@property(nonatomic) CodexPacerSnapshotFree snapshotFree;
@property(nonatomic) CodexPacerActionHandler actionHandler;
@property(nonatomic) BOOL popupEnabled;
@property(nonatomic) BOOL panelClosing;
@property(nonatomic, copy) NSString *displayLanguage;
- (void)closePanelAnimated:(BOOL)animated;
@end

@implementation CPNativeMenuBarController
+ (instancetype)sharedController {
    static CPNativeMenuBarController *controller;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        controller = [[CPNativeMenuBarController alloc] init];
    });
    return controller;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _popupEnabled = YES;
        _displayLanguage = @"zh-CN";
        _panel = [[CPMenuBarPanel alloc] initWithContentRect:NSMakeRect(0, 0, CPPanelWidth, CPPanelHeight)
                                                   styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                     backing:NSBackingStoreBuffered
                                                       defer:NO];
        _panel.opaque = NO;
        _panel.backgroundColor = NSColor.clearColor;
        _panel.hasShadow = YES;
        _panel.releasedWhenClosed = NO;
        _panel.hidesOnDeactivate = NO;
        _panel.worksWhenModal = YES;
        _panel.level = NSPopUpMenuWindowLevel;
        _panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                    NSWindowCollectionBehaviorFullScreenAuxiliary |
                                    NSWindowCollectionBehaviorTransient |
                                    NSWindowCollectionBehaviorIgnoresCycle |
                                    NSWindowCollectionBehaviorStationary;
        _panel.acceptsMouseMovedEvents = YES;
    }
    return self;
}

- (BOOL)usesChineseInterface {
    return CPIsChineseLanguage(self.displayLanguage);
}

- (void)configureWithSnapshotProvider:(CodexPacerSnapshotProvider)provider
                         snapshotFree:(CodexPacerSnapshotFree)freeCallback
                         actionHandler:(CodexPacerActionHandler)handler {
    self.snapshotProvider = provider;
    self.snapshotFree = freeCallback;
    self.actionHandler = handler;
    if (!self.statusItem) {
        NSStatusBar *statusBar = NSStatusBar.systemStatusBar;
        CGFloat height = statusBar.thickness > 0.0 ? statusBar.thickness : 24.0;
        self.statusItem = [statusBar statusItemWithLength:42.0];
        self.statusView = [[CPStatusItemView alloc] initWithFrame:NSMakeRect(0, 0, 42.0, height)];
        self.statusView.delegate = self;
        NSStatusBarButton *button = self.statusItem.button;
        button.target = self;
        button.action = @selector(statusItemButtonPressed:);
        [button sendActionOn:NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown];
        button.image = [[NSImage alloc] initWithSize:NSMakeSize(1.0, 1.0)];
        button.imagePosition = NSNoImage;
        [button addSubview:self.statusView];
        self.statusItem.visible = NO;
    }
}

- (void)updateVisible:(BOOL)visible popupEnabled:(BOOL)popupEnabled showLogo:(BOOL)showLogo apiValueTitle:(NSString *)apiValueTitle liveMetricLabel:(NSString *)liveMetricLabel liveMetricValue:(NSString *)liveMetricValue tooltip:(NSString *)tooltip {
    if (!self.statusItem || !self.statusView) return;
    self.popupEnabled = popupEnabled;
    NSStatusBarButton *button = self.statusItem.button;
    self.statusItem.visible = visible;
    if (!visible) [self closePanelAnimated:NO];
    NSString *resolvedTooltip = tooltip.length > 0 ? tooltip : @"Codex Pacer";
    self.statusView.toolTip = resolvedTooltip;
    button.toolTip = resolvedTooltip;
    self.statusView.apiValueTitle = apiValueTitle ?: @"";
    self.statusView.liveMetricLabel = liveMetricLabel ?: @"";
    self.statusView.liveMetricValue = liveMetricValue ?: @"";
    self.statusView.showLogo = showLogo && self.statusView.liveMetricValue.length == 0;
    if (self.statusView.showLogo) {
        NSImage *icon = [NSApp.applicationIconImage copy];
        icon.size = NSMakeSize(18.0, 18.0);
        icon.template = YES;
        self.statusView.icon = icon;
    } else {
        self.statusView.icon = nil;
    }

    CGFloat width = ceil([self.statusView preferredWidth]);
    CGFloat height = NSStatusBar.systemStatusBar.thickness > 0.0 ? NSStatusBar.systemStatusBar.thickness : 24.0;
    self.statusItem.length = width;
    self.statusView.frame = NSMakeRect(0, 0, width, height);
    self.statusView.needsDisplay = YES;
}

- (void)statusItemButtonPressed:(id)sender {
    NSEvent *event = NSApp.currentEvent;
    if (event.type == NSEventTypeRightMouseDown || event.type == NSEventTypeRightMouseUp) {
        [self statusItemViewRightClicked:self.statusView];
        return;
    }
    [self statusItemViewLeftClicked:self.statusView];
}

- (void)statusItemViewLeftClicked:(id)sender {
    if (self.panel.visible) {
        [self closePanel];
        return;
    }
    if (!self.popupEnabled) {
        if (self.actionHandler) self.actionHandler("open_dashboard");
        return;
    }
    [self showPanelWithForceRefresh:NO];
}

- (void)statusItemViewRightClicked:(id)sender {
    [self showContextMenu];
}

- (void)showContextMenu {
    [self closePanelAnimated:NO];
    NSString *ignoredError = nil;
    [self snapshotWithForceRefresh:NO error:&ignoredError];
    BOOL zh = [self usesChineseInterface];
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"Codex Pacer"];
    NSMenuItem *showItem = [[NSMenuItem alloc] initWithTitle:CPT(zh, @"显示主面板", @"Show dashboard") action:@selector(openDashboard:) keyEquivalent:@""];
    showItem.target = self;
    [menu addItem:showItem];
    NSMenuItem *settingsItem = [[NSMenuItem alloc] initWithTitle:CPT(zh, @"设置", @"Settings") action:@selector(openSettings:) keyEquivalent:@""];
    settingsItem.target = self;
    [menu addItem:settingsItem];
    NSMenuItem *refreshItem = [[NSMenuItem alloc] initWithTitle:CPT(zh, @"刷新", @"Refresh") action:@selector(refreshButtonPressed:) keyEquivalent:@""];
    refreshItem.target = self;
    [menu addItem:refreshItem];
    [menu addItem:NSMenuItem.separatorItem];
    NSMenuItem *quitItem = [[NSMenuItem alloc] initWithTitle:CPT(zh, @"退出 Codex Pacer", @"Quit Codex Pacer") action:@selector(quitApp:) keyEquivalent:@""];
    quitItem.target = self;
    [menu addItem:quitItem];
    NSStatusBarButton *button = self.statusItem.button;
    NSPoint menuPoint = NSMakePoint(NSMidX(button.bounds), NSMinY(button.bounds) - 2.0);
    [menu popUpMenuPositioningItem:nil atLocation:menuPoint inView:button];
}

- (void)showPanelWithForceRefresh:(BOOL)forceRefresh {
    [self rebuildPanelWithForceRefresh:forceRefresh];
    NSStatusBarButton *button = self.statusItem.button;
    if (!button) return;
    NSRect finalFrame = [self panelFrameForButton:button];
    NSRect openingFrame = CPPanelAnimationFrame(finalFrame);
    self.panelClosing = NO;
    self.panel.alphaValue = 0.0;
    [self.panel setFrame:openingFrame display:YES animate:NO];
    [self.panel orderFrontRegardless];
    [self.panel makeKeyWindow];
    [self installEventMonitors];
    __weak CPNativeMenuBarController *weakSelf = self;
    dispatch_async(dispatch_get_main_queue(), ^{
        CPNativeMenuBarController *strongSelf = weakSelf;
        if (!strongSelf || !strongSelf.panel.visible || strongSelf.panelClosing) return;
        [strongSelf.panel setFrame:openingFrame display:YES animate:NO];
        strongSelf.panel.alphaValue = 0.0;
        [strongSelf.panel.contentView displayIfNeeded];
        [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
            context.duration = CPPanelOpenAnimationDuration;
            context.timingFunction = [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseOut];
            context.allowsImplicitAnimation = YES;
            strongSelf.panel.animator.alphaValue = 1.0;
            [strongSelf.panel.animator setFrame:finalFrame display:YES];
        } completionHandler:^{
            if (!strongSelf.panel.visible || strongSelf.panelClosing) return;
            strongSelf.panel.alphaValue = 1.0;
            [strongSelf.panel setFrame:finalFrame display:YES animate:NO];
        }];
    });
}

- (NSRect)panelFrameForButton:(NSStatusBarButton *)button {
    NSWindow *buttonWindow = button.window;
    NSRect buttonFrame = NSZeroRect;
    if (buttonWindow) {
        buttonFrame = [button convertRect:button.bounds toView:nil];
        buttonFrame = [buttonWindow convertRectToScreen:buttonFrame];
    } else {
        NSPoint mouse = NSEvent.mouseLocation;
        buttonFrame = NSMakeRect(mouse.x, mouse.y, 1.0, 1.0);
    }

    NSScreen *screen = buttonWindow.screen ?: NSScreen.mainScreen;
    NSRect visibleFrame = screen ? screen.visibleFrame : NSMakeRect(0, 0, CPPanelWidth, CPPanelHeight + 44.0);
    CGFloat margin = 8.0;
    CGFloat x = NSMidX(buttonFrame) - CPPanelWidth / 2.0;
    x = fmax(NSMinX(visibleFrame) + margin, fmin(x, NSMaxX(visibleFrame) - CPPanelWidth - margin));

    CGFloat y = NSMinY(buttonFrame) - CPPanelHeight - margin;
    if (y < NSMinY(visibleFrame) + margin) {
        y = NSMaxY(buttonFrame) + margin;
    }
    y = fmax(NSMinY(visibleFrame) + margin, fmin(y, NSMaxY(visibleFrame) - CPPanelHeight - margin));
    return NSMakeRect(x, y, CPPanelWidth, CPPanelHeight);
}

- (void)closePanel {
    [self closePanelAnimated:YES];
}

- (void)closePanelAnimated:(BOOL)animated {
    [self removeEventMonitors];
    if (!self.panel.visible) {
        self.panelClosing = NO;
        self.panel.alphaValue = 1.0;
        return;
    }
    if (!animated) {
        self.panelClosing = NO;
        self.panel.alphaValue = 1.0;
        [self.panel orderOut:nil];
        return;
    }
    if (self.panelClosing) return;

    self.panelClosing = YES;
    NSRect currentFrame = self.panel.frame;
    NSRect dismissedFrame = CPPanelAnimationFrame(currentFrame);
    [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
        context.duration = CPPanelCloseAnimationDuration;
        context.timingFunction = [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseIn];
        context.allowsImplicitAnimation = YES;
        self.panel.animator.alphaValue = 0.0;
        [self.panel.animator setFrame:dismissedFrame display:YES];
    } completionHandler:^{
        [self.panel orderOut:nil];
        self.panel.alphaValue = 1.0;
        [self.panel setFrame:currentFrame display:NO animate:NO];
        self.panelClosing = NO;
    }];
}

- (void)refreshPanel {
    if (!self.panel.visible) return;
    [self rebuildPanelWithForceRefresh:NO];
}

- (void)installEventMonitors {
    [self removeEventMonitors];
    __weak CPNativeMenuBarController *weakSelf = self;
    self.localEventMonitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown | NSEventMaskKeyDown
                                                                   handler:^NSEvent *(NSEvent *event) {
        CPNativeMenuBarController *strongSelf = weakSelf;
        if (!strongSelf) return event;
        if (event.type == NSEventTypeKeyDown) {
            if (event.keyCode == 53) {
                [strongSelf closePanel];
                return nil;
            }
            return event;
        }
        if (event.window == strongSelf.panel || event.window == strongSelf.statusItem.button.window) return event;
        [strongSelf closePanel];
        return event;
    }];
    self.globalEventMonitor = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown
                                                                     handler:^(__unused NSEvent *event) {
        CPNativeMenuBarController *strongSelf = weakSelf;
        [strongSelf closePanel];
    }];
}

- (void)removeEventMonitors {
    if (self.localEventMonitor) {
        [NSEvent removeMonitor:self.localEventMonitor];
        self.localEventMonitor = nil;
    }
    if (self.globalEventMonitor) {
        [NSEvent removeMonitor:self.globalEventMonitor];
        self.globalEventMonitor = nil;
    }
}

- (void)rebuildPanelWithForceRefresh:(BOOL)forceRefresh {
    NSString *error = nil;
    NSDictionary *snapshot = [self snapshotWithForceRefresh:forceRefresh error:&error];
    self.panel.contentView = [self buildPanelViewWithSnapshot:snapshot error:error];
}

- (NSDictionary *)snapshotWithForceRefresh:(BOOL)forceRefresh error:(NSString **)errorOut {
    BOOL zh = [self usesChineseInterface];
    if (!self.snapshotProvider) {
        if (errorOut) *errorOut = CPT(zh, @"原生快照桥接不可用。", @"Native snapshot bridge is unavailable.");
        return nil;
    }
    char *raw = self.snapshotProvider(forceRefresh);
    if (!raw) {
        if (errorOut) *errorOut = CPT(zh, @"Rust 快照提供器没有返回数据。", @"Rust snapshot provider returned no data.");
        return nil;
    }
    NSString *json = [NSString stringWithUTF8String:raw] ?: @"";
    if (self.snapshotFree) self.snapshotFree(raw);
    NSData *data = [json dataUsingEncoding:NSUTF8StringEncoding];
    NSError *error = nil;
    id object = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:&error] : nil;
    NSDictionary *snapshot = CPAsDictionary(object);
    NSString *bridgeError = CPString(snapshot, @"error");
    if (bridgeError.length > 0) {
        if (errorOut) *errorOut = bridgeError;
        return nil;
    }
    if (![object isKindOfClass:NSDictionary.class]) {
        if (errorOut) *errorOut = error.localizedDescription ?: CPT(zh, @"快照 JSON 无效。", @"Snapshot JSON is invalid.");
        return nil;
    }
    NSString *language = CPString(snapshot, @"displayLanguage");
    if (language.length > 0) self.displayLanguage = language;
    return snapshot;
}

- (NSView *)buildPanelViewWithSnapshot:(NSDictionary *)snapshot error:(NSString *)error {
    BOOL zh = snapshot ? CPIsChineseSnapshot(snapshot) : [self usesChineseInterface];
    NSVisualEffectView *root = [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, CPPanelWidth, CPPanelHeight)];
    if (@available(macOS 10.14, *)) {
        root.material = (NSVisualEffectMaterial)13;
    } else {
        root.material = (NSVisualEffectMaterial)2;
    }
    root.blendingMode = NSVisualEffectBlendingModeBehindWindow;
    root.state = NSVisualEffectStateActive;
    root.wantsLayer = YES;
    root.layer.cornerRadius = 22.0;
    root.layer.masksToBounds = YES;

    NSStackView *stack = [[NSStackView alloc] init];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.distribution = NSStackViewDistributionGravityAreas;
    stack.spacing = 12.0;
    stack.edgeInsets = NSEdgeInsetsMake(14.0, 14.0, 14.0, 14.0);
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [root addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:root.leadingAnchor],
        [stack.trailingAnchor constraintEqualToAnchor:root.trailingAnchor],
        [stack.topAnchor constraintEqualToAnchor:root.topAnchor],
        [stack.bottomAnchor constraintLessThanOrEqualToAnchor:root.bottomAnchor],
    ]];

    [stack addArrangedSubview:[self buildHeaderForSnapshot:snapshot]];

    if (error.length > 0 || !snapshot) {
        NSView *card = CPCardView(20.0);
        [card.heightAnchor constraintEqualToConstant:260.0].active = YES;
        [card.widthAnchor constraintEqualToConstant:CPPanelWidth - 28.0].active = YES;
        NSStackView *empty = [[NSStackView alloc] init];
        empty.orientation = NSUserInterfaceLayoutOrientationVertical;
        empty.alignment = NSLayoutAttributeCenterX;
        empty.spacing = 10.0;
        empty.translatesAutoresizingMaskIntoConstraints = NO;
        [card addSubview:empty];
        [NSLayoutConstraint activateConstraints:@[
            [empty.centerXAnchor constraintEqualToAnchor:card.centerXAnchor],
            [empty.centerYAnchor constraintEqualToAnchor:card.centerYAnchor],
            [empty.widthAnchor constraintLessThanOrEqualToConstant:330.0],
        ]];
        [empty addArrangedSubview:CPLabel(CPT(zh, @"暂无弹窗快照", @"No panel snapshot"), [NSFont systemFontOfSize:17 weight:NSFontWeightSemibold], CPColor(0.98, 0.93, 0.84, 1.0), 1)];
        [empty addArrangedSubview:CPLabel(error ?: CPT(zh, @"请刷新或扫描 Codex 使用记录，以生成菜单栏指标。", @"Refresh or scan Codex usage to generate menu bar metrics."), [NSFont systemFontOfSize:12 weight:NSFontWeightMedium], CPColor(0.73, 0.69, 0.62, 1.0), 3)];
        [stack addArrangedSubview:card];
        return root;
    }

    [stack addArrangedSubview:[self buildQuotaRow:snapshot]];
    [stack addArrangedSubview:[self buildTrendCard:snapshot]];
    if ([CPNumber(snapshot, @"showResetTimeline") boolValue]) {
        [stack addArrangedSubview:[self buildResetRow:snapshot]];
    }
    [stack addArrangedSubview:[self buildModuleGrid:snapshot]];
    return root;
}

- (NSView *)buildHeaderForSnapshot:(NSDictionary *)snapshot {
    BOOL zh = snapshot ? CPIsChineseSnapshot(snapshot) : [self usesChineseInterface];
    NSStackView *header = [[NSStackView alloc] init];
    header.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    header.alignment = NSLayoutAttributeCenterY;
    header.spacing = 10.0;
    header.translatesAutoresizingMaskIntoConstraints = NO;
    [header.widthAnchor constraintEqualToConstant:CPPanelWidth - 28.0].active = YES;

    NSImageView *icon = [[NSImageView alloc] init];
    icon.image = NSApp.applicationIconImage;
    icon.imageScaling = NSImageScaleProportionallyUpOrDown;
    icon.wantsLayer = YES;
    icon.layer.cornerRadius = 11.0;
    icon.layer.backgroundColor = CPColor(0.47, 0.77, 1.0, 0.13).CGColor;
    icon.translatesAutoresizingMaskIntoConstraints = NO;
    [icon.widthAnchor constraintEqualToConstant:34.0].active = YES;
    [icon.heightAnchor constraintEqualToConstant:34.0].active = YES;
    [header addArrangedSubview:icon];

    NSStackView *titles = [[NSStackView alloc] init];
    titles.orientation = NSUserInterfaceLayoutOrientationVertical;
    titles.alignment = NSLayoutAttributeLeading;
    titles.spacing = 3.0;
    titles.translatesAutoresizingMaskIntoConstraints = NO;
    [titles addArrangedSubview:CPLabel(@"Codex Pacer", [NSFont systemFontOfSize:15 weight:NSFontWeightSemibold], CPColor(0.98, 0.93, 0.84, 1.0), 1)];
    NSString *bucket = CPBucketLabel(CPString(snapshot, @"selectedBucket") ?: @"month", zh);
    NSString *fetched = CPRelativeTime(CPString(snapshot, @"fetchedAt"), zh);
    [titles addArrangedSubview:CPLabel([NSString stringWithFormat:@"%@ · %@ · %@", bucket, fetched, CPT(zh, @"原生面板", @"native panel")], [NSFont systemFontOfSize:11 weight:NSFontWeightMedium], CPColor(0.73, 0.69, 0.62, 1.0), 1)];
    [header addArrangedSubview:titles];
    [titles setContentHuggingPriority:NSLayoutPriorityDefaultLow forOrientation:NSLayoutConstraintOrientationHorizontal];

    [header addArrangedSubview:[NSView new]];

    NSNumber *showActions = CPNumber(snapshot, @"showActions");
    if (!showActions || showActions.boolValue) {
        [header addArrangedSubview:[self iconButtonWithSymbol:@"chart.xyaxis.line" action:@selector(openDashboard:) tooltip:CPT(zh, @"打开主面板", @"Open dashboard")]];
        [header addArrangedSubview:[self iconButtonWithSymbol:@"gearshape" action:@selector(openSettings:) tooltip:CPT(zh, @"设置", @"Settings")]];
        [header addArrangedSubview:[self iconButtonWithSymbol:@"arrow.clockwise" action:@selector(refreshButtonPressed:) tooltip:CPT(zh, @"刷新", @"Refresh")]];
    }
    return header;
}

- (NSButton *)iconButtonWithSymbol:(NSString *)symbol action:(SEL)action tooltip:(NSString *)tooltip {
    NSButton *button = [[NSButton alloc] initWithFrame:NSZeroRect];
    button.bordered = NO;
    button.bezelStyle = NSBezelStyleRegularSquare;
    button.target = self;
    button.action = action;
    button.toolTip = tooltip;
    button.translatesAutoresizingMaskIntoConstraints = NO;
    button.wantsLayer = YES;
    button.layer.cornerRadius = 11.0;
    button.layer.backgroundColor = CPColor(1.0, 1.0, 1.0, 0.065).CGColor;
    if (@available(macOS 11.0, *)) {
        NSImage *image = [NSImage imageWithSystemSymbolName:symbol accessibilityDescription:tooltip];
        image.template = YES;
        button.image = image;
        button.imagePosition = NSImageOnly;
    } else {
        button.title = @"•";
    }
    [button.widthAnchor constraintEqualToConstant:32.0].active = YES;
    [button.heightAnchor constraintEqualToConstant:32.0].active = YES;
    return button;
}

- (NSView *)buildQuotaRow:(NSDictionary *)snapshot {
    BOOL zh = CPIsChineseSnapshot(snapshot);
    NSStackView *row = [[NSStackView alloc] init];
    row.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    row.spacing = 12.0;
    row.translatesAutoresizingMaskIntoConstraints = NO;
    [row.widthAnchor constraintEqualToConstant:CPPanelWidth - 28.0].active = YES;
    [row addArrangedSubview:[self quotaCardWithTitle:CPT(zh, @"5小时", @"5h") quota:CPAsDictionary(snapshot[@"quota5h"]) accent:CPColor(1.0, 0.70, 0.42, 1.0)]];
    [row addArrangedSubview:[self quotaCardWithTitle:CPT(zh, @"7天", @"7d") quota:CPAsDictionary(snapshot[@"quota7d"]) accent:CPColor(0.47, 0.77, 1.0, 1.0)]];
    return row;
}

- (NSView *)quotaCardWithTitle:(NSString *)title quota:(NSDictionary *)quota accent:(NSColor *)accent {
    NSView *card = CPCardView(20.0);
    [card.widthAnchor constraintEqualToConstant:(CPPanelWidth - 40.0) / 2.0].active = YES;
    [card.heightAnchor constraintEqualToConstant:158.0].active = YES;
    CPQuotaRingView *ring = [[CPQuotaRingView alloc] initWithFrame:NSZeroRect];
    ring.remainingPercent = CPNumber(quota, @"remainingPercent") ? CPNumber(quota, @"remainingPercent").doubleValue : 0.0;
    ring.timePercent = [self remainingTimePercentForQuota:quota];
    ring.accentColor = accent;
    ring.title = title;
    ring.translatesAutoresizingMaskIntoConstraints = NO;
    [card addSubview:ring];
    [NSLayoutConstraint activateConstraints:@[
        [ring.leadingAnchor constraintEqualToAnchor:card.leadingAnchor constant:8.0],
        [ring.trailingAnchor constraintEqualToAnchor:card.trailingAnchor constant:-8.0],
        [ring.topAnchor constraintEqualToAnchor:card.topAnchor constant:8.0],
        [ring.bottomAnchor constraintEqualToAnchor:card.bottomAnchor constant:-8.0],
    ]];
    return card;
}

- (CGFloat)remainingTimePercentForQuota:(NSDictionary *)quota {
    NSDate *reset = CPDateFromString(CPString(quota, @"resetsAt"));
    NSDate *start = CPDateFromString(CPString(quota, @"windowStart"));
    if (!reset || !start || [reset timeIntervalSinceDate:start] <= 0) return 0.0;
    NSTimeInterval total = [reset timeIntervalSinceDate:start];
    NSTimeInterval remaining = [reset timeIntervalSinceNow];
    return MAX(0.0, MIN(100.0, remaining / total * 100.0));
}

- (NSView *)buildTrendCard:(NSDictionary *)snapshot {
    BOOL zh = CPIsChineseSnapshot(snapshot);
    NSView *card = CPCardView(20.0);
    [card.widthAnchor constraintEqualToConstant:CPPanelWidth - 28.0].active = YES;
    [card.heightAnchor constraintEqualToConstant:184.0].active = YES;

    NSStackView *stack = [[NSStackView alloc] init];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.spacing = 6.0;
    stack.edgeInsets = NSEdgeInsetsMake(10.0, 10.0, 8.0, 10.0);
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [card addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:card.leadingAnchor],
        [stack.trailingAnchor constraintEqualToAnchor:card.trailingAnchor],
        [stack.topAnchor constraintEqualToAnchor:card.topAnchor],
        [stack.bottomAnchor constraintEqualToAnchor:card.bottomAnchor],
    ]];

    NSStackView *header = [[NSStackView alloc] init];
    header.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    header.alignment = NSLayoutAttributeCenterY;
    header.spacing = 8.0;
    header.translatesAutoresizingMaskIntoConstraints = NO;
    [header.widthAnchor constraintEqualToConstant:CPPanelWidth - 48.0].active = YES;
    [header addArrangedSubview:CPLabel(CPT(zh, @"7天额度趋势", @"Seven-day usage trend"), [NSFont systemFontOfSize:12 weight:NSFontWeightSemibold], CPColor(0.98, 0.93, 0.84, 1.0), 1)];
    NSView *spacer = [NSView new];
    [spacer setContentHuggingPriority:NSLayoutPriorityDefaultLow forOrientation:NSLayoutConstraintOrientationHorizontal];
    [header addArrangedSubview:spacer];

    NSArray *trend = CPAsArray(snapshot[@"quotaTrend7d"]);
    NSDictionary *quota7d = CPAsDictionary(snapshot[@"quota7d"]);
    double apiValue = [self sevenDayApiValueForTrend:trend snapshot:snapshot quota:quota7d];
    [header addArrangedSubview:[self trendBadgeWithEmoji:@"💵" label:CPT(zh, @"7天价值", @"7d value") value:CPUSD(apiValue) borderColor:CPColor(1.0, 0.70, 0.42, 0.42) emptyValue:CPT(zh, @"无数据", @"No data")]];
    NSDictionary *speed = CPAsDictionary(snapshot[@"suggestedSpeed7d"]);
    NSString *speedValue = CPString(speed, @"displayValue");
    if (speedValue.length > 0) {
        NSString *emoji = CPString(speed, @"emoji") ?: @"";
        [header addArrangedSubview:[self trendBadgeWithEmoji:emoji label:nil value:speedValue borderColor:[self speedBadgeBorderColor:CPString(speed, @"status")] emptyValue:CPT(zh, @"无数据", @"No data")]];
    }
    [stack addArrangedSubview:header];

    CPTrendChartView *chart = [[CPTrendChartView alloc] initWithFrame:NSZeroRect];
    chart.points = trend;
    chart.quota = quota7d;
    chart.fetchedAt = CPString(snapshot, @"liveQuotaFetchedAt") ?: CPString(snapshot, @"fetchedAt");
    chart.emptyLabel = CPT(zh, @"暂无 7 天额度趋势", @"No seven-day usage trend");
    chart.translatesAutoresizingMaskIntoConstraints = NO;
    [chart.heightAnchor constraintEqualToConstant:108.0].active = YES;
    [chart.widthAnchor constraintEqualToConstant:CPPanelWidth - 48.0].active = YES;
    [stack addArrangedSubview:chart];

    NSStackView *legend = [[NSStackView alloc] init];
    legend.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    legend.alignment = NSLayoutAttributeCenterY;
    legend.spacing = 12.0;
    legend.translatesAutoresizingMaskIntoConstraints = NO;
    [legend.widthAnchor constraintLessThanOrEqualToConstant:CPPanelWidth - 48.0].active = YES;
    [legend addArrangedSubview:[self trendLegendItemWithTitle:CPT(zh, @"剩余", @"Remaining") kind:@"remaining"]];
    [legend addArrangedSubview:[self trendLegendItemWithTitle:CPT(zh, @"参考", @"Reference") kind:@"reference"]];
    [legend addArrangedSubview:[self trendLegendItemWithTitle:CPT(zh, @"当前", @"Current") kind:@"current"]];
    [stack addArrangedSubview:legend];
    return card;
}

- (NSView *)trendBadgeWithEmoji:(NSString *)emoji label:(NSString *)label value:(NSString *)value borderColor:(NSColor *)borderColor emptyValue:(NSString *)emptyValue {
    NSView *badge = [[NSView alloc] initWithFrame:NSZeroRect];
    badge.translatesAutoresizingMaskIntoConstraints = NO;
    badge.wantsLayer = YES;
    badge.layer.cornerRadius = 12.0;
    badge.layer.backgroundColor = CPColor(7.0 / 255.0, 13.0 / 255.0, 24.0 / 255.0, 0.76).CGColor;
    badge.layer.borderColor = (borderColor ?: CPColor(1.0, 1.0, 1.0, 0.12)).CGColor;
    badge.layer.borderWidth = 1.0;
    [badge.heightAnchor constraintEqualToConstant:26.0].active = YES;

    NSStackView *stack = [[NSStackView alloc] init];
    stack.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    stack.alignment = NSLayoutAttributeCenterY;
    stack.spacing = 5.0;
    stack.edgeInsets = NSEdgeInsetsMake(0, 8.0, 0, 8.0);
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [badge addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:badge.leadingAnchor],
        [stack.trailingAnchor constraintEqualToAnchor:badge.trailingAnchor],
        [stack.topAnchor constraintEqualToAnchor:badge.topAnchor],
        [stack.bottomAnchor constraintEqualToAnchor:badge.bottomAnchor],
    ]];

    if (emoji.length > 0) {
        [stack addArrangedSubview:CPLabel(emoji, [NSFont systemFontOfSize:11 weight:NSFontWeightRegular], CPColor(0.98, 0.93, 0.84, 1.0), 1)];
    }
    if (label.length > 0) {
        [stack addArrangedSubview:CPLabel(label, [NSFont systemFontOfSize:10 weight:NSFontWeightMedium], CPColor(0.73, 0.69, 0.62, 1.0), 1)];
    }
    [stack addArrangedSubview:CPLabel(value ?: (emptyValue ?: @"No data"), [NSFont monospacedDigitSystemFontOfSize:12 weight:NSFontWeightSemibold], CPColor(1.0, 0.97, 0.91, 1.0), 1)];
    return badge;
}

- (NSColor *)speedBadgeBorderColor:(NSString *)status {
    if ([status isEqualToString:@"fast"]) return CPColor(1.0, 0.70, 0.42, 0.36);
    if ([status isEqualToString:@"healthy"]) return CPColor(0.55, 1.0, 0.75, 0.32);
    if ([status isEqualToString:@"slow"]) return CPColor(0.47, 0.77, 1.0, 0.36);
    return CPColor(1.0, 1.0, 1.0, 0.12);
}

- (NSView *)trendLegendItemWithTitle:(NSString *)title kind:(NSString *)kind {
    NSStackView *item = [[NSStackView alloc] init];
    item.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    item.alignment = NSLayoutAttributeCenterY;
    item.spacing = 5.0;
    item.translatesAutoresizingMaskIntoConstraints = NO;

    CPChartLegendMarkerView *mark = [[CPChartLegendMarkerView alloc] initWithFrame:NSZeroRect];
    mark.translatesAutoresizingMaskIntoConstraints = NO;
    mark.kind = kind;
    if ([kind isEqualToString:@"current"]) {
        [mark.widthAnchor constraintEqualToConstant:18.0].active = YES;
        [mark.heightAnchor constraintEqualToConstant:10.0].active = YES;
    } else {
        [mark.widthAnchor constraintEqualToConstant:20.0].active = YES;
        [mark.heightAnchor constraintEqualToConstant:10.0].active = YES;
    }
    [item addArrangedSubview:mark];
    [item addArrangedSubview:CPLabel(title, [NSFont systemFontOfSize:10 weight:NSFontWeightMedium], CPColor(0.73, 0.69, 0.62, 1.0), 1)];
    return item;
}

- (double)sevenDayApiValueForTrend:(NSArray *)trend snapshot:(NSDictionary *)snapshot quota:(NSDictionary *)quota {
    NSDate *windowStart = CPDateFromString(CPString(quota, @"windowStart"));
    NSDate *current = CPDateFromString(CPString(snapshot, @"liveQuotaFetchedAt")) ?: CPDateFromString(CPString(snapshot, @"fetchedAt"));
    double latest = 0.0;
    for (id item in trend) {
        NSDictionary *point = CPAsDictionary(item);
        NSDate *date = CPDateFromString(CPString(point, @"timestamp"));
        NSNumber *value = CPNumber(point, @"cumulativeApiValueUsd");
        if (!date || !value) continue;
        if (windowStart && [date compare:windowStart] == NSOrderedAscending) continue;
        if (current && [date compare:current] == NSOrderedDescending) continue;
        latest = value.doubleValue;
    }
    return latest;
}

- (NSView *)buildResetRow:(NSDictionary *)snapshot {
    BOOL zh = CPIsChineseSnapshot(snapshot);
    NSStackView *row = [[NSStackView alloc] init];
    row.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    row.spacing = 8.0;
    row.translatesAutoresizingMaskIntoConstraints = NO;
    [row.widthAnchor constraintEqualToConstant:CPPanelWidth - 28.0].active = YES;
    [row addArrangedSubview:[self pillWithLabel:CPT(zh, @"5小时重置", @"5h reset") value:CPShortDateTime(CPString(CPAsDictionary(snapshot[@"quota5h"]), @"resetsAt"), zh)]];
    [row addArrangedSubview:[self pillWithLabel:CPT(zh, @"7天重置", @"7d reset") value:CPShortDateTime(CPString(CPAsDictionary(snapshot[@"quota7d"]), @"resetsAt"), zh)]];
    return row;
}

- (NSView *)pillWithLabel:(NSString *)label value:(NSString *)value {
    NSView *card = CPCardView(16.0);
    [card.widthAnchor constraintEqualToConstant:(CPPanelWidth - 36.0) / 2.0].active = YES;
    [card.heightAnchor constraintEqualToConstant:46.0].active = YES;
    NSStackView *stack = [[NSStackView alloc] init];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.spacing = 2.0;
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [stack addArrangedSubview:CPLabel(label, [NSFont systemFontOfSize:10 weight:NSFontWeightMedium], CPColor(0.73, 0.69, 0.62, 1.0), 1)];
    [stack addArrangedSubview:CPLabel(value, [NSFont systemFontOfSize:13 weight:NSFontWeightSemibold], CPColor(0.98, 0.93, 0.84, 1.0), 1)];
    [card addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:card.leadingAnchor constant:12.0],
        [stack.trailingAnchor constraintEqualToAnchor:card.trailingAnchor constant:-12.0],
        [stack.centerYAnchor constraintEqualToAnchor:card.centerYAnchor],
    ]];
    return card;
}

- (NSView *)buildModuleGrid:(NSDictionary *)snapshot {
    BOOL zh = CPIsChineseSnapshot(snapshot);
    NSStackView *grid = [[NSStackView alloc] init];
    grid.orientation = NSUserInterfaceLayoutOrientationVertical;
    grid.spacing = 8.0;
    grid.translatesAutoresizingMaskIntoConstraints = NO;
    [grid.widthAnchor constraintEqualToConstant:CPPanelWidth - 28.0].active = YES;

    NSString *bucket = CPBucketLabel(CPString(snapshot, @"selectedBucket"), zh);
    NSDictionary *descriptors = @{
        @"api_value": @[CPT(zh, @"当前范围", @"Selected bucket"), CPUSD([CPNumber(snapshot, @"apiValueSelectedBucket") doubleValue]), bucket ?: @""],
        @"token_count": @[CPT(zh, @"Token 数", @"Tokens"), CPCompactInteger([CPNumber(snapshot, @"totalTokensSelectedBucket") longLongValue], zh), bucket ?: @""],
        @"conversation_count": @[CPT(zh, @"对话数", @"Conversations"), [NSString stringWithFormat:@"%lld", [CPNumber(snapshot, @"conversationCountSelectedBucket") longLongValue]], bucket ?: @""],
        @"payoff_ratio": @[CPT(zh, @"回本率", @"Payoff"), CPPercentRatio([CPNumber(snapshot, @"payoffRatio") doubleValue]), CPT(zh, @"API 价值 / 订阅费", @"API value / subscription")],
        @"scan_freshness": @[CPT(zh, @"上次扫描", @"Last scan"), CPRelativeTime(CPString(snapshot, @"lastScanCompletedAt"), zh), CPShortDateTime(CPString(snapshot, @"lastScanCompletedAt"), zh)],
        @"live_quota_freshness": @[CPT(zh, @"额度刷新", @"Live quota"), CPRelativeTime(CPString(snapshot, @"liveQuotaFetchedAt"), zh), CPShortDateTime(CPString(snapshot, @"liveQuotaFetchedAt"), zh)],
    };
    NSArray *fallbackModuleIds = @[
        @"api_value",
        @"token_count",
        @"conversation_count",
        @"payoff_ratio",
        @"scan_freshness",
        @"live_quota_freshness",
    ];
    NSArray *configuredModuleIds = CPAsArray(snapshot[@"visibleModules"]);
    NSArray *moduleIds = configuredModuleIds.count > 0 ? configuredModuleIds : fallbackModuleIds;
    NSMutableArray *modules = [NSMutableArray array];
    for (id moduleId in moduleIds) {
        if (![moduleId isKindOfClass:NSString.class]) continue;
        NSArray *values = descriptors[moduleId];
        if (values) [modules addObject:values];
    }
    if (modules.count == 0) {
        for (NSString *moduleId in fallbackModuleIds) {
            NSArray *values = descriptors[moduleId];
            if (values) [modules addObject:values];
        }
    }

    for (NSUInteger index = 0; index < modules.count; index += 2) {
        NSStackView *row = [[NSStackView alloc] init];
        row.orientation = NSUserInterfaceLayoutOrientationHorizontal;
        row.spacing = 8.0;
        row.translatesAutoresizingMaskIntoConstraints = NO;
        [row addArrangedSubview:[self moduleCard:modules[index]]];
        if (index + 1 < modules.count) {
            [row addArrangedSubview:[self moduleCard:modules[index + 1]]];
        }
        [grid addArrangedSubview:row];
    }
    return grid;
}

- (NSView *)moduleCard:(NSArray *)values {
    NSView *card = CPCardView(16.0);
    [card.widthAnchor constraintEqualToConstant:(CPPanelWidth - 36.0) / 2.0].active = YES;
    [card.heightAnchor constraintEqualToConstant:66.0].active = YES;
    NSStackView *stack = [[NSStackView alloc] init];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.spacing = 2.0;
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [stack addArrangedSubview:CPLabel(values.count > 0 ? values[0] : @"", [NSFont systemFontOfSize:10 weight:NSFontWeightMedium], CPColor(0.73, 0.69, 0.62, 1.0), 1)];
    [stack addArrangedSubview:CPLabel(values.count > 1 ? values[1] : @"", [NSFont systemFontOfSize:18 weight:NSFontWeightBold], CPColor(0.98, 0.93, 0.84, 1.0), 1)];
    [stack addArrangedSubview:CPLabel(values.count > 2 ? values[2] : @"", [NSFont systemFontOfSize:10 weight:NSFontWeightMedium], CPColor(0.73, 0.69, 0.62, 1.0), 1)];
    [card addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:card.leadingAnchor constant:12.0],
        [stack.trailingAnchor constraintEqualToAnchor:card.trailingAnchor constant:-12.0],
        [stack.centerYAnchor constraintEqualToAnchor:card.centerYAnchor],
    ]];
    return card;
}

- (void)openDashboard:(id)sender {
    if (self.actionHandler) self.actionHandler("open_dashboard");
}

- (void)openSettings:(id)sender {
    if (self.actionHandler) self.actionHandler("open_settings");
}

- (void)refreshButtonPressed:(id)sender {
    if (self.actionHandler) {
        self.actionHandler("refresh");
    } else {
        [self rebuildPanelWithForceRefresh:YES];
    }
}

- (void)quitApp:(id)sender {
    if (self.actionHandler) {
        self.actionHandler("quit");
    } else {
        [NSApp terminate:nil];
    }
}
@end

static void CPOnMain(dispatch_block_t block) {
    if ([NSThread isMainThread]) {
        block();
    } else {
        dispatch_async(dispatch_get_main_queue(), block);
    }
}

void codex_pacer_macos_menu_bar_configure(CodexPacerSnapshotProvider snapshotProvider,
                                           CodexPacerSnapshotFree snapshotFree,
                                           CodexPacerActionHandler actionHandler) {
    CPOnMain(^{
        [[CPNativeMenuBarController sharedController] configureWithSnapshotProvider:snapshotProvider
                                                                       snapshotFree:snapshotFree
                                                                       actionHandler:actionHandler];
    });
}

void codex_pacer_macos_menu_bar_update(bool visible,
                                        bool popupEnabled,
                                        bool showLogo,
                                        const char *apiValueTitle,
                                        const char *liveMetricLabel,
                                        const char *liveMetricValue,
                                        const char *tooltip) {
    NSString *apiValueTitleString = apiValueTitle ? ([NSString stringWithUTF8String:apiValueTitle] ?: @"") : @"";
    NSString *liveMetricLabelString = liveMetricLabel ? ([NSString stringWithUTF8String:liveMetricLabel] ?: @"") : @"";
    NSString *liveMetricValueString = liveMetricValue ? ([NSString stringWithUTF8String:liveMetricValue] ?: @"") : @"";
    NSString *tooltipString = tooltip ? ([NSString stringWithUTF8String:tooltip] ?: @"Codex Pacer") : @"Codex Pacer";
    CPOnMain(^{
        [[CPNativeMenuBarController sharedController] updateVisible:visible
                                                      popupEnabled:popupEnabled
                                                           showLogo:showLogo
                                                       apiValueTitle:apiValueTitleString
                                                    liveMetricLabel:liveMetricLabelString
                                                    liveMetricValue:liveMetricValueString
                                                            tooltip:tooltipString];
    });
}

void codex_pacer_macos_menu_bar_close_popover(void) {
    CPOnMain(^{
        [[CPNativeMenuBarController sharedController] closePanel];
    });
}

void codex_pacer_macos_menu_bar_refresh_popover(void) {
    CPOnMain(^{
        [[CPNativeMenuBarController sharedController] refreshPanel];
    });
}
