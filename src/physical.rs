//! Physical streams.
//!
//! This modules defines the components of physical streams as described in the
//! [Tydi specification].
//!
//! This modules defines the following types:
//! - [`Complexity`] the interface complexity level.
//!   [Reference](https://abs-tudelft.github.io/tydi/specification/physical.html#complexity-c)
//! - [`Fields`] the fields of a physical stream.
//!   [Reference](https://abs-tudelft.github.io/tydi/specification/physical.html#element-content-e-and-usertransfer-content-u)
//! - [`PhysicalStream`] a physical stream.
//!   [Reference](https://abs-tudelft.github.io/tydi/specification/physical.html#physical-stream-specification)
//! - [`SignalList`] a signal list for the signals in a physical stream.
//!   [Reference](https://abs-tudelft.github.io/tydi/specification/physical.html#signals)
//!
//! # Examples
//!
//! ## Minimal example
//!
//! ```rust
//! use tydi::physical::{PhysicalStream, SignalList};
//!
//! // Construct a new physical stream with two elements, named "a" and "b".
//! // The stream has two elements lanes, no dimensionality data, a complexity
//! // of (major) level 2, and no user fields.
//! let physical_stream =
//!     PhysicalStream::try_new(vec![("a", 4), ("b", 8)], 2, 0, 2, vec![])?;
//!
//! // Get the signal list for the physical stream.
//! let signal_list = physical_stream.signal_list();
//!
//! // Validate the signal list bit count. It should equal to (4 + 8) * 2.
//! assert_eq!(signal_list.bit_count(), 24);
//!
//! // For a complexity level of 8 there should be 4 additional signals.
//! // (2 `strb`, 1 `stai`, 1 `endi`).
//! let signal_list =
//!     SignalList::from(
//!         PhysicalStream::try_new(vec![("a", 4), ("b", 8)], 2, 0, 8, vec![])?
//!     );
//! assert_eq!(signal_list.bit_count(), 28);
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`Complexity`]: ./struct.Complexity.html
//! [`Fields`]: ./struct.Fields.html
//! [`PhysicalStream`]: ./struct.PhysicalStream.html
//! [`SignalMap`]: ./struct.SignalMap.html
//! [Tydi specification]: https://abs-tudelft.github.io/tydi/specification/physical.html

use std::str::FromStr;
use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt,
};

use indexmap::IndexMap;

use crate::traits::Identify;
use crate::{util::log2_ceil, Error, NonNegative, PathName, Positive, Result};

/// Positive number of bits.
pub type BitCount = Positive;

/// Interface complexity level.
///
/// This logical stream parameter specifies the guarantees a source makes about
/// how elements are transferred. Equivalently, it specifies the assumptions a
/// sink can safely make.
///
/// # Examples
///
/// ```rust
/// use tydi::physical::Complexity;
///
/// let c3 = Complexity::new_major(3);
/// let c30 = Complexity::new(vec![3, 0])?;
/// let c31 = Complexity::new(vec![3, 1])?;
/// let c4 = Complexity::new_major(4);
///
/// assert_eq!(c3, c30);
/// assert!(c3 < c31);
/// assert!(c31 < c4);
///
/// assert_eq!(c31.to_string(), "3.1");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [Reference]
///
/// [Reference]: https://abs-tudelft.github.io/tydi/specification/physical.html#complexity-c
#[derive(Debug, Clone)]
pub struct Complexity {
    /// The complexity level.
    level: Vec<NonNegative>,
}

impl Default for Complexity {
    fn default() -> Self {
        Complexity { level: vec![4] }
    }
}

impl PartialEq for Complexity {
    /// A complexity number is higher than another when the leftmost integer is
    /// greater, and lower when the leftmost integer is lower. If the leftmost
    /// integer is equal, the next integer is checked recursively. If one
    /// complexity number has more entries than another, the shorter number is
    /// padded with zeros on the right.
    fn eq(&self, other: &Self) -> bool {
        (0..self.level.len().max(other.level.len()))
            .all(|idx| self.level.get(idx).unwrap_or(&0) == other.level.get(idx).unwrap_or(&0))
    }
}

impl Eq for Complexity {}

impl PartialOrd for Complexity {
    fn partial_cmp(&self, other: &Complexity) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Complexity {
    /// A complexity number is higher than another when the leftmost integer is
    /// greater, and lower when the leftmost integer is lower. If the leftmost
    /// integer is equal, the next integer is checked recursively. If one
    /// complexity number has more entries than another, the shorter number is
    /// padded with zeros on the right.
    fn cmp(&self, other: &Complexity) -> Ordering {
        (0..self.level.len().max(other.level.len()))
            .map(|idx| {
                (
                    self.level.get(idx).unwrap_or(&0),
                    other.level.get(idx).unwrap_or(&0),
                )
            })
            .fold(None, |ord, (i, j)| match ord {
                Some(ord) => Some(ord),
                None => {
                    if i == j {
                        None
                    } else {
                        Some(i.cmp(j))
                    }
                }
            })
            .unwrap_or(Ordering::Equal)
    }
}

impl From<NonNegative> for Complexity {
    /// Convert a NonNegative into complexity with the NonNegative as major version.
    fn from(major: NonNegative) -> Self {
        Complexity::new_major(major)
    }
}

impl TryFrom<Vec<NonNegative>> for Complexity {
    type Error = Error;
    /// Try to convert a vector of NonNegative into a complexity. Returns an
    /// error when the provided vector is empty.
    fn try_from(level: Vec<NonNegative>) -> Result<Self> {
        Complexity::new(level)
    }
}

impl FromStr for Complexity {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Complexity::new(
            // split string into string slices
            s.split('.')
                // convert slices to nonnegatives after trimming whitespace
                .map(|d| d.trim().parse::<NonNegative>())
                // convert to result with vector of nonnegatives
                .collect::<std::result::Result<Vec<_>, std::num::ParseIntError>>()
                // convert potential error to tydi error
                .map_err(|e| Error::InvalidArgument(e.to_string()))?,
        )
    }
}

impl Complexity {
    /// Constructs a new Complexity with provided level. Returns an error when
    /// the provided level iterator is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tydi::physical::Complexity;
    ///
    /// let c = Complexity::new(vec![1, 2, 3, 4])?;
    /// assert!(Complexity::new(vec![]).is_err());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(level: impl IntoIterator<Item = NonNegative>) -> Result<Self> {
        let level = level.into_iter().collect::<Vec<NonNegative>>();
        if level.is_empty() {
            Err(Error::InvalidArgument(
                "complexity level cannot be empty".to_string(),
            ))
        } else {
            Ok(Complexity { level })
        }
    }

    /// Constructs a new Complexity with provided level as major version.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tydi::physical::Complexity;
    ///
    /// let c = Complexity::new_major(4);
    ///
    /// assert_eq!(c, Complexity::new(vec![4])?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new_major(level: NonNegative) -> Self {
        Complexity { level: vec![level] }
    }

    /// Returns the level of this Complexity.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tydi::physical::Complexity;
    ///
    /// let c = Complexity::new(vec![3, 14])?;
    /// assert_eq!(c.level(), &[3, 14]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn level(&self) -> &[NonNegative] {
        self.level.as_ref()
    }

    /// Returns the major version of this Complexity level.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tydi::physical::Complexity;
    ///
    /// let c = Complexity::new(vec![3, 14])?;
    /// assert_eq!(c.major(), 3);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    pub fn major(&self) -> NonNegative {
        self.level[0]
    }
}

impl fmt::Display for Complexity {
    /// Display a complexity level as a version number. The levels are
    /// separated by periods.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tydi::physical::Complexity;
    ///
    /// let c = Complexity::new(vec![3, 14])?;
    /// assert_eq!(c.to_string(), "3.14");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = String::new();
        let mut level = self.level.iter().map(|x| x.to_string());
        if let Some(x) = level.next() {
            result.push_str(&x);
            level.for_each(|x| {
                result.push('.');
                result.push_str(&x);
            });
        }
        write!(f, "{}", result)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fields(IndexMap<PathName, BitCount>);

impl Fields {
    pub fn new(iter: impl IntoIterator<Item = (PathName, BitCount)>) -> Result<Self> {
        let fields = iter.into_iter();
        let (lower, upper) = fields.size_hint();
        let mut map = IndexMap::with_capacity(upper.unwrap_or(lower));

        for (path_name, bit_count) in fields {
            map.insert(path_name, bit_count)
                .map(|_| -> Result<()> { Err(Error::UnexpectedDuplicate) })
                .transpose()?;
        }

        Ok(Fields(map))
    }

    pub(crate) fn new_empty() -> Self {
        Fields(IndexMap::new())
    }

    pub(crate) fn insert(&mut self, path_name: PathName, bit_count: BitCount) -> Result<()> {
        self.0
            .insert(path_name, bit_count)
            .map(|_| -> Result<()> { Err(Error::UnexpectedDuplicate) })
            .transpose()?;
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PathName, &BitCount)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &PathName> {
        self.0.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &BitCount> {
        self.0.values()
    }
}

impl<'a> IntoIterator for &'a Fields {
    type Item = (&'a PathName, &'a BitCount);
    type IntoIter = indexmap::map::Iter<'a, PathName, BitCount>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Physical stream.
///
/// A physical stream carries a stream of elements, dimensionality information
/// for said elements, and (optionally) user-defined transfer information from
/// a source to a sink.
///
/// [Reference]
///
/// [Reference]: https://abs-tudelft.github.io/tydi/specification/physical.html#physical-stream-specification
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalStream {
    /// Element content.
    element_fields: Fields,
    /// Number of element lanes.
    element_lanes: Positive,
    /// Dimensionality.
    dimensionality: NonNegative,
    /// Complexity.
    complexity: Complexity,
    /// User-defined transfer content.
    user: Fields,
}

impl PhysicalStream {
    pub fn try_new<T, U>(
        element_fields: T,
        element_lanes: usize,
        dimensionality: usize,
        complexity: impl Into<Complexity>,
        user: T,
    ) -> Result<Self>
    where
        T: IntoIterator<Item = (U, usize)>,
        U: TryInto<PathName, Error = Error>,
    {
        let element_fields = Fields::new(
            element_fields
                .into_iter()
                .map(|(path_name, bit_count)| {
                    (
                        path_name.try_into(),
                        Positive::new(bit_count as NonNegative),
                    )
                })
                .map(|(path_name, bit_count)| match (path_name, bit_count) {
                    (Ok(path_name), Some(bit_count)) => Ok((path_name, bit_count)),
                    (Err(e), _) => Err(e),
                    (_, None) => Err(Error::InvalidArgument(
                        "element lanes cannot be zero".to_string(),
                    )),
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        let element_lanes = Positive::new(element_lanes as NonNegative)
            .ok_or_else(|| Error::InvalidArgument("element lanes cannot be zero".to_string()))?;
        let dimensionality = dimensionality as NonNegative;
        let complexity = complexity.into();
        let user = Fields::new(
            user.into_iter()
                .map(|(path_name, bit_count)| {
                    (
                        path_name.try_into(),
                        Positive::new(bit_count as NonNegative),
                    )
                })
                .map(|(path_name, bit_count)| match (path_name, bit_count) {
                    (Ok(path_name), Some(bit_count)) => Ok((path_name, bit_count)),
                    (Err(e), _) => Err(e),
                    (_, None) => Err(Error::InvalidArgument(
                        "element lanes cannot be zero".to_string(),
                    )),
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        Ok(PhysicalStream::new(
            element_fields,
            element_lanes,
            dimensionality,
            complexity,
            user,
        ))
    }
    /// Constructs a new PhysicalStream using provided arguments. Returns an
    /// error when provided argument are not valid.
    pub fn new(
        element_fields: impl Into<Fields>,
        element_lanes: Positive,
        dimensionality: NonNegative,
        complexity: impl Into<Complexity>,
        user: impl Into<Fields>,
    ) -> Self {
        PhysicalStream {
            element_fields: element_fields.into(),
            element_lanes,
            dimensionality,
            complexity: complexity.into(),
            user: user.into(),
        }
    }

    /// Returns the element fields in this physical stream.
    pub fn element_fields(&self) -> &Fields {
        &self.element_fields
    }

    /// Returns the number of element lanes in this physical stream.
    pub fn element_lanes(&self) -> Positive {
        self.element_lanes
    }

    /// Returns the dimensionality of this physical stream.
    pub fn dimensionality(&self) -> NonNegative {
        self.dimensionality
    }

    /// Returns the complexity of this physical stream.
    pub fn complexity(&self) -> &Complexity {
        &self.complexity
    }

    /// Returns the user fields in this physical stream.
    pub fn user(&self) -> &Fields {
        &self.user
    }

    /// Returns the bit count of the data (element) fields in this physical
    /// stream. The bit count is equal to the combined bit count of all fields
    /// multiplied by the number of lanes.
    pub fn data_bit_count(&self) -> NonNegative {
        self.element_fields
            .values()
            .map(|b| b.get())
            .sum::<NonNegative>()
            * self.element_lanes.get()
    }

    /// Returns the number of last bits in this physical stream. The number of
    /// last bits equals the dimensionality.
    pub fn last_bit_count(&self) -> NonNegative {
        self.dimensionality
    }

    /// Returns the number of `stai` (start index) bits in this physical
    /// stream.
    pub fn stai_bit_count(&self) -> NonNegative {
        if self.complexity.major() >= 6 && self.element_lanes.get() > 1 {
            log2_ceil(self.element_lanes)
        } else {
            0
        }
    }

    /// Returns the number of `endi` (end index) bits in this physical stream.
    pub fn endi_bit_count(&self) -> NonNegative {
        if (self.complexity.major() >= 5 || self.dimensionality >= 1)
            && self.element_lanes.get() > 1
        {
            log2_ceil(self.element_lanes)
        } else {
            0
        }
    }

    /// Returns the number of `strb` (strobe) bits in this physical stream.
    pub fn strb_bit_count(&self) -> NonNegative {
        if self.complexity.major() >= 7 || self.dimensionality >= 1 {
            self.element_lanes.get()
        } else {
            0
        }
    }

    /// Returns the bit count of the user fields in this physical stream.
    pub fn user_bit_count(&self) -> NonNegative {
        self.user.values().map(|b| b.get()).sum::<NonNegative>()
    }

    /// Returns the signal list for this physical stream.
    pub fn signal_list(&self) -> SignalList {
        let opt = |x| if x == 0 { None } else { Some(x) };
        SignalList {
            data: opt(self.data_bit_count()),
            last: opt(self.last_bit_count()),
            stai: opt(self.stai_bit_count()),
            endi: opt(self.endi_bit_count()),
            strb: opt(self.strb_bit_count()),
            user: opt(self.user_bit_count()),
        }
    }

    /// Returns the combined bit count of all signals in this physical stream.
    /// This excludes the `valid` and `ready` signals.
    pub fn bit_count(&self) -> NonNegative {
        self.data_bit_count()
            + self.last_bit_count()
            + self.stai_bit_count()
            + self.endi_bit_count()
            + self.strb_bit_count()
            + self.user_bit_count()
    }
}

impl From<&PhysicalStream> for SignalList {
    fn from(physical_stream: &PhysicalStream) -> SignalList {
        physical_stream.signal_list()
    }
}

impl From<PhysicalStream> for SignalList {
    fn from(physical_stream: PhysicalStream) -> SignalList {
        physical_stream.signal_list()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Origin {
    Source,
    Sink,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Width {
    /// Non-vectorized single bit.
    Scalar,
    /// Vectorized multiple bits.
    Vector(NonNegative),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    name: String,
    origin: Origin,
    width: Width,
}

impl Identify for Signal {
    fn identifier(&self) -> &str {
        self.name.as_str()
    }
}

impl Signal {
    /// Returns a vector-style signal if the input width is Some(NonNegative)
    pub fn opt_vec(
        name: impl Into<String>,
        origin: Origin,
        width: Option<NonNegative>,
    ) -> Option<Signal> {
        width.map(|w| Signal {
            name: name.into(),
            origin,
            width: Width::Vector(w),
        })
    }

    /// Returns a vector-style signal.
    pub fn vec(name: impl Into<String>, origin: Origin, width: Positive) -> Signal {
        Signal {
            name: name.into(),
            origin,
            width: Width::Vector(width.get()),
        }
    }

    /// Returns a single bit non-vector style signal.
    pub fn bit(name: impl Into<String>, origin: Origin) -> Signal {
        Signal {
            name: name.into(),
            origin,
            width: Width::Scalar,
        }
    }

    /// Returns whether the signal is reversed w.r.t. the source
    pub fn reversed(&self) -> bool {
        self.origin == Origin::Sink
    }

    pub fn origin(&self) -> Origin {
        self.origin
    }

    pub fn width(&self) -> Width {
        self.width
    }

    pub fn with_name(&self, name: String) -> Signal {
        Signal {
            name,
            origin: self.origin,
            width: self.width,
        }
    }
}

/// Signal list for the signals in a physical stream.
///
/// A signal list can be constructed from a [`PhysicalStream`] using the
/// [`signal_list`] method or using the `From`/`Into` trait implementation.
///
/// [Reference]
///
/// [`PhysicalStream`]: ./struct.PhysicalStream.html
/// [`signal_list`]: ./struct.PhysicalStream.html#method.signal_list
/// [Reference]: https://abs-tudelft.github.io/tydi/specification/physical.html#signals
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SignalList {
    data: Option<NonNegative>,
    last: Option<NonNegative>,
    stai: Option<NonNegative>,
    endi: Option<NonNegative>,
    strb: Option<NonNegative>,
    user: Option<NonNegative>,
}

impl SignalList {
    /// Returns the valid signal.
    pub fn valid(&self) -> Signal {
        Signal {
            name: "valid".to_string(),
            origin: Origin::Source,
            width: Width::Scalar,
        }
    }

    /// Returns the ready signal.
    pub fn ready(&self) -> Signal {
        Signal {
            name: "ready".to_string(),
            origin: Origin::Sink,
            width: Width::Scalar,
        }
    }

    /// Returns the `data` signal, if applicable for this PhysicalStream.
    pub fn data(&self) -> Option<Signal> {
        Signal::opt_vec("data", Origin::Source, self.data)
    }

    /// Returns the `last` signal, if applicable for this PhysicalStream.
    pub fn last(&self) -> Option<Signal> {
        Signal::opt_vec("last", Origin::Source, self.last)
    }

    /// Returns the `stai` signal, if applicable for this PhysicalStream.
    pub fn stai(&self) -> Option<Signal> {
        Signal::opt_vec("stai", Origin::Source, self.stai)
    }

    /// Returns the `endi` signal, if applicable for this PhysicalStream.
    pub fn endi(&self) -> Option<Signal> {
        Signal::opt_vec("endi", Origin::Source, self.endi)
    }

    /// Returns the `strb` signal, if applicable for this PhysicalStream.
    pub fn strb(&self) -> Option<Signal> {
        Signal::opt_vec("strb", Origin::Source, self.strb)
    }

    /// Returns the `user` signal, if applicable for this PhysicalStream.
    pub fn user(&self) -> Option<Signal> {
        Signal::opt_vec("user", Origin::Source, self.user)
    }

    /// Returns the bit count of all combined signals in this map.
    pub fn opt_bit_count(&self) -> Option<NonNegative> {
        match self.data.unwrap_or(0)
            + self.last.unwrap_or(0)
            + self.stai.unwrap_or(0)
            + self.endi.unwrap_or(0)
            + self.strb.unwrap_or(0)
            + self.user.unwrap_or(0)
        {
            0 => None,
            x => Some(x),
        }
    }

    /// Returns the bit count of all combined signals in this map.
    pub fn bit_count(&self) -> NonNegative {
        self.opt_bit_count().unwrap_or(0)
    }
}

impl<'a> IntoIterator for &'a SignalList {
    type Item = Signal;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        [
            Some(self.valid()),
            Some(self.ready()),
            self.data(),
            self.last(),
            self.stai(),
            self.endi(),
            self.strb(),
            self.user(),
        ]
        .iter()
        .filter(|o| o.is_some())
        .map(|s| s.clone().unwrap())
        .collect::<Vec<_>>()
        .into_iter()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn complexity() -> Result<()> {
        use std::convert::TryInto;

        let empty = Complexity::new(vec![]);
        assert_eq!(
            empty.unwrap_err().to_string(),
            "Invalid argument: complexity level cannot be empty"
        );
        assert_eq!(
            Complexity::try_from(vec![]).unwrap_err().to_string(),
            "Invalid argument: complexity level cannot be empty"
        );

        let c = Complexity::new_major(0);
        let c3 = Complexity::new_major(3);
        let c30 = Complexity::new(vec![3, 0])?;
        let c31 = Complexity::new(vec![3, 1])?;
        let c311 = Complexity::new(vec![3, 1, 1])?;
        let c32 = Complexity::new(vec![3, 2])?;
        let c4 = Complexity::new_major(4);
        let c400 = Complexity::new(vec![4, 0, 0])?;
        let c401 = Complexity::new(vec![4, 0, 1])?;
        assert!(c < c3);
        assert!(c3 < c31);
        assert!(!(c3 < c30));
        assert!(!(c3 > c30));
        assert_eq!(c3, c30);
        assert!(c31 < c311);
        assert!(c311 < c32);
        assert!(c32 < c4);
        assert_eq!(c4, c400);
        assert_eq!(c400, c4);
        assert!(!(c400 > c4));
        assert!(!(c400 < c4));
        assert!(c400 < c401);
        assert!(c4 < c401);
        assert_eq!(c3, 3.into());
        assert_eq!(c401, vec![4, 0, 1].try_into()?);

        assert_eq!(c3.to_string(), "3");
        assert_eq!(c31.to_string(), "3.1");

        assert_eq!(c3.major(), 3);
        assert_eq!(c31.major(), 3);
        assert_eq!(c4.major(), 4);

        assert_eq!(c4.level(), &[4]);
        assert_eq!(c400.level(), &[4, 0, 0]);
        Ok(())
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn physical_stream() -> Result<()> {
        let physical_stream = PhysicalStream::new(
            Fields::new(vec![
                ("a".try_into()?, BitCount::new(8).unwrap()),
                ("b".try_into()?, BitCount::new(16).unwrap()),
                ("c".try_into()?, BitCount::new(1).unwrap()),
            ])?,
            Positive::new(3).unwrap(),
            4,
            8,
            Fields::new(vec![("user".try_into()?, BitCount::new(1).unwrap())])?,
        );

        let mut element = physical_stream.element_fields().iter();
        assert_eq!(
            element.next(),
            Some((&("a".try_into()?), &BitCount::new(8).unwrap()))
        );
        assert_eq!(
            element.next(),
            Some((&("b".try_into()?), &BitCount::new(16).unwrap()))
        );
        assert_eq!(
            element.next(),
            Some((&("c".try_into()?), &BitCount::new(1).unwrap()))
        );
        assert_eq!(element.next(), None);
        assert_eq!(physical_stream.element_lanes(), Positive::new(3).unwrap());
        assert_eq!(physical_stream.dimensionality(), 4);
        assert_eq!(physical_stream.complexity(), &Complexity::new_major(8));
        assert_eq!(
            physical_stream.user().iter().next().unwrap(),
            (&("user".try_into()?), &BitCount::new(1).unwrap())
        );
        assert_eq!(physical_stream.bit_count(), 87);
        assert_eq!(physical_stream.data_bit_count(), (8 + 16 + 1) * 3);
        assert_eq!(physical_stream.last_bit_count(), 4);
        assert_eq!(physical_stream.stai_bit_count(), 2);
        assert_eq!(physical_stream.endi_bit_count(), 2);
        assert_eq!(physical_stream.strb_bit_count(), 3);
        assert_eq!(physical_stream.user_bit_count(), 1);
        assert_eq!(
            physical_stream.signal_list(),
            SignalList {
                data: Some(75),
                last: Some(4),
                stai: Some(2),
                endi: Some(2),
                strb: Some(3),
                user: Some(1)
            }
        );

        // let physical_stream = PhysicalStream::new(vec![(Some("a"), 8)], 1, 0, 0, vec![])?;
        let physical_stream = PhysicalStream::new(
            Fields::new(vec![("a".try_into()?, BitCount::new(8).unwrap())])?,
            Positive::new(1).unwrap(),
            0,
            0,
            Fields::new(vec![])?,
        );

        assert_eq!(physical_stream.element_fields().iter().count(), 1);
        assert_eq!(physical_stream.element_lanes(), Positive::new(1).unwrap());
        assert_eq!(physical_stream.dimensionality(), 0);
        assert_eq!(physical_stream.complexity(), &Complexity::new_major(0));
        assert_eq!(physical_stream.user().iter().next(), None);
        assert_eq!(physical_stream.bit_count(), 8);
        assert_eq!(physical_stream.data_bit_count(), 8);
        assert_eq!(physical_stream.last_bit_count(), 0);
        assert_eq!(physical_stream.stai_bit_count(), 0);
        assert_eq!(physical_stream.endi_bit_count(), 0);
        assert_eq!(physical_stream.strb_bit_count(), 0);
        assert_eq!(physical_stream.user_bit_count(), 0);
        assert_eq!(
            physical_stream.signal_list(),
            SignalList {
                data: Some(8),
                last: None,
                stai: None,
                endi: None,
                strb: None,
                user: None
            }
        );

        Ok(())
    }

    #[test]
    fn signal_list() -> Result<()> {
        let physical_stream = PhysicalStream::new(
            Fields::new(vec![
                ("a".try_into()?, BitCount::new(3).unwrap()),
                ("b".try_into()?, BitCount::new(2).unwrap()),
            ])?,
            Positive::new(2).unwrap(),
            3,
            8,
            Fields::new(vec![])?,
        );

        let signal_list = SignalList::from(&physical_stream);
        assert_eq!(physical_stream.bit_count(), 17);
        assert_eq!(physical_stream.data_bit_count(), 2 * (3 + 2));
        assert_eq!(physical_stream.last_bit_count(), 3);
        assert_eq!(physical_stream.stai_bit_count(), 1);
        assert_eq!(physical_stream.endi_bit_count(), 1);
        assert_eq!(physical_stream.strb_bit_count(), 2);
        assert_eq!(physical_stream.user_bit_count(), 0);

        assert_eq!(
            Width::Vector(physical_stream.data_bit_count()),
            signal_list.data().unwrap().width
        );
        assert_eq!(
            Width::Vector(physical_stream.last_bit_count()),
            signal_list.last().unwrap().width
        );
        assert_eq!(
            Width::Vector(physical_stream.stai_bit_count()),
            signal_list.stai().unwrap().width
        );
        assert_eq!(
            Width::Vector(physical_stream.endi_bit_count()),
            signal_list.endi().unwrap().width
        );
        assert_eq!(
            Width::Vector(physical_stream.strb_bit_count()),
            signal_list.strb().unwrap().width
        );
        assert_eq!(
            Width::Vector(physical_stream.user_bit_count()),
            Width::Vector(0)
        );

        assert_eq!(signal_list.opt_bit_count(), Some(17));
        assert_eq!(signal_list.bit_count(), 17);
        assert_eq!(signal_list, SignalList::from(physical_stream));

        assert_eq!(
            signal_list.into_iter().collect::<Vec<_>>(),
            vec![
                Signal::bit("valid", Origin::Source),
                Signal::bit("ready", Origin::Sink),
                Signal::opt_vec("data", Origin::Source, Some(10)).unwrap(),
                Signal::opt_vec("last", Origin::Source, Some(3)).unwrap(),
                Signal::opt_vec("stai", Origin::Source, Some(1)).unwrap(),
                Signal::opt_vec("endi", Origin::Source, Some(1)).unwrap(),
                Signal::opt_vec("strb", Origin::Source, Some(2)).unwrap(),
                // ("user", 0) ommitted
            ]
        );

        Ok(())
    }
}
