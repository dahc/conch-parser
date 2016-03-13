//! Defines an interfaces to receive parse data and construct ASTs.
//!
//! This allows the parser to remain agnostic of the required source
//! representation, and frees up the library user to substitute their own.
//! If one does not require a custom AST representation, this module offers
//! a reasonable default builder implementation.
//!
//! If a custom AST representation is required you will need to implement
//! the `Builder` trait for your AST. Otherwise you can provide the `DefaultBuilder`
//! struct to the parser if you wish to use the default AST implementation.

use std::rc::Rc;
use syntax::ast::{Arithmetic, Command, CompoundCommand, ComplexWord, GuardBodyPair, Parameter,
                  ParameterSubstitution, Redirect, SimpleCommand, SimpleWord, TopLevelWord, Word};
use syntax::parse::Result;

/// An indicator to the builder of how complete commands are separated.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SeparatorKind {
    /// A semicolon appears between commands, normally indicating a sequence.
    Semi,
    /// An ampersand appears between commands, normally indicating an asyncronous job.
    Amp,
    /// A newline (and possibly a comment) appears at the end of a command before the next.
    Newline(Newline),
    /// The command was delimited by a token (e.g. a compound command delimiter) or
    /// the end of input, but is *not* followed by another sequential command.
    Other,
}

/// An indicator to the builder whether an `AND` or `OR` command was parsed.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum AndOrKind {
    /// An `AND` command was parsed, normally indicating the second should run if the first succeeds.
    /// Corresponds to the `&&` command separator.
    And,
    /// An `OR` command was parsed, normally indicating the second should run if the first fails.
    /// Corresponds to the `||` command separator.
    Or,
}

/// An indicator to the builder whether a `while` or `until` command was parsed.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum LoopKind {
    /// A `while` command was parsed, normally indicating the loop's body should be run
    /// while the guard's exit status is successful.
    While,
    /// An `until` command was parsed, normally indicating the loop's body should be run
    /// until the guard's exit status becomes successful.
    Until,
}

/// Parsed fragments relating to a shell `if` command.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct IfFragments<C> {
    /// A list of conditionals branches.
    pub conditionals: Vec<GuardBodyPair<C>>,
    /// The `else` branch, if any,
    pub else_part: Option<Vec<C>>,
}

/// Parsed fragments relating to a shell `for` command.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ForFragments<W, C> {
    /// The name of the variable to which each of the words will be bound.
    pub var: String,
    /// Any comments that appear after the variable declaration.
    pub post_var_comments: Vec<Newline>,
    /// A group of words to iterate over, if present.
    pub words: Option<Vec<W>>,
    /// Any comments that appear after the `words` declaration (if it exists).
    pub post_words_comments: Option<Vec<Newline>>,
    /// The body to be invoked for every iteration.
    pub body: Vec<C>,
}

/// Parsed fragments relating to a shell `case` command.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CaseFragments<W, C> {
    /// The word to be matched against.
    pub word: W,
    /// The comments appearing after the word to match but before the `in` reserved word.
    pub post_word_comments: Vec<Newline>,
    /// The different arms in the case command. Each arm has a number of pattern alternatives,
    /// and a body of commands to run if any pattern matches.
    pub arms: Vec<(CasePatternFragments<W>, Vec<C>)>,
    /// The comments appearing after the last arm but before the `esac` reserved word
    pub post_arms_comments: Vec<Newline>,
}

/// Parsed fragments relating to patterns in a shell `case` command.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CasePatternFragments<W> {
    /// Comments appearing after a previous arm, but before the start of a pattern.
    pub pre_pattern_comments: Vec<Newline>,
    /// Pattern alternatives which all correspond to the same case arm.
    pub pattern_alternatives: Vec<W>,
    /// Comments appearing after the patterns but before the start of the case arm.
    pub post_pattern_comments: Vec<Newline>,
}

/// An indicator to the builder what kind of complex word was parsed.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ComplexWordKind<C> {
    /// Several distinct words concatenated together.
    Concat(Vec<WordKind<C>>),
    /// A regular word.
    Single(WordKind<C>),
}

/// An indicator to the builder what kind of word was parsed.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WordKind<C> {
    /// A regular word.
    Simple(SimpleWordKind<C>),
    /// List of words concatenated within double quotes.
    DoubleQuoted(Vec<SimpleWordKind<C>>),
    /// List of words concatenated within single quotes. Virtually
    /// identical as a literal, but makes a distinction between the two.
    SingleQuoted(String),
}

/// An indicator to the builder what kind of simple word was parsed.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SimpleWordKind<C> {
    /// A non-special literal word.
    Literal(String),
    /// Access of a value inside a parameter, e.g. `$foo` or `$$`.
    Param(Parameter),
    /// A parameter substitution, e.g. `${param-word}`.
    Subst(ParameterSubstitutionKind<C, ComplexWordKind<C>>),
    /// Represents the standard output of some command, e.g. \`echo foo\`.
    CommandSubst(Vec<C>),
    /// A token which normally has a special meaning is treated as a literal
    /// because it was escaped, typically with a backslash, e.g. `\"`.
    Escaped(String),
    /// Represents `*`, useful for handling pattern expansions.
    Star,
    /// Represents `?`, useful for handling pattern expansions.
    Question,
    /// Represents `[`, useful for handling pattern expansions.
    SquareOpen,
    /// Represents `]`, useful for handling pattern expansions.
    SquareClose,
    /// Represents `~`, useful for handling tilde expansions.
    Tilde,
    /// Represents `:`, useful for handling tilde expansions.
    Colon,
}

/// Represents redirecting a command's file descriptors.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RedirectKind<W> {
    /// Open a file for reading, e.g. `[n]< file`.
    Read(Option<u16>, W),
    /// Open a file for writing after truncating, e.g. `[n]> file`.
    Write(Option<u16>, W),
    /// Open a file for reading and writing, e.g. `[n]<> file`.
    ReadWrite(Option<u16>, W),
    /// Open a file for writing, appending to the end, e.g. `[n]>> file`.
    Append(Option<u16>, W),
    /// Open a file for writing, failing if the `noclobber` shell option is set, e.g. `[n]>| file`.
    Clobber(Option<u16>, W),
    /// Lines contained in the source that should be provided by as input to a file descriptor.
    Heredoc(Option<u16>, W),
    /// Duplicate a file descriptor for reading, e.g. `[n]<& [n|-]`.
    DupRead(Option<u16>, W),
    /// Duplicate a file descriptor for writing, e.g. `[n]>& [n|-]`.
    DupWrite(Option<u16>, W),
}

/// Represents the type of parameter that was parsed
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParameterSubstitutionKind<C, W> {
    /// Returns the standard output of running a command, e.g. `$(cmd)`
    Command(Vec<C>),
    /// Returns the length of the value of a parameter, e.g. ${#param}
    Len(Parameter),
    /// Returns the resulting value of an arithmetic subsitution, e.g. `$(( x++ ))`
    Arith(Option<Arithmetic>),
    /// Use a provided value if the parameter is null or unset, e.g.
    /// `${param:-[word]}`.
    /// The boolean indicates the presence of a `:`, and that if the parameter has
    /// a null value, that situation should be treated as if the parameter is unset.
    Default(bool, Parameter, Option<Box<W>>),
    /// Assign a provided value to the parameter if it is null or unset,
    /// e.g. `${param:=[word]}`.
    /// The boolean indicates the presence of a `:`, and that if the parameter has
    /// a null value, that situation should be treated as if the parameter is unset.
    Assign(bool, Parameter, Option<Box<W>>),
    /// If the parameter is null or unset, an error should result with the provided
    /// message, e.g. `${param:?[word]}`.
    /// The boolean indicates the presence of a `:`, and that if the parameter has
    /// a null value, that situation should be treated as if the parameter is unset.
    Error(bool, Parameter, Option<Box<W>>),
    /// If the parameter is NOT null or unset, a provided word will be used,
    /// e.g. `${param:+[word]}`.
    /// The boolean indicates the presence of a `:`, and that if the parameter has
    /// a null value, that situation should be treated as if the parameter is unset.
    Alternative(bool, Parameter, Option<Box<W>>),
    /// Remove smallest suffix pattern, e.g. `${param%pattern}`
    RemoveSmallestSuffix(Parameter, Option<Box<W>>),
    /// Remove largest suffix pattern, e.g. `${param%%pattern}`
    RemoveLargestSuffix(Parameter, Option<Box<W>>),
    /// Remove smallest prefix pattern, e.g. `${param#pattern}`
    RemoveSmallestPrefix(Parameter, Option<Box<W>>),
    /// Remove largest prefix pattern, e.g. `${param##pattern}`
    RemoveLargestPrefix(Parameter, Option<Box<W>>),
}

/// Represents a parsed newline, more specifically, the presense of a comment
/// immediately preceeding the newline.
///
/// Since shell comments are usually treated as a newline, they can be present
/// anywhere a newline can be as well. Thus if it is desired to retain comments
/// they can be optionally attached to a parsed newline.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Newline(pub Option<String>);

/// A trait which defines an interface which the parser defined in the `parse` module
/// uses to delegate Abstract Syntax Tree creation. The methods defined here correspond
/// to their respectively named methods on the parser, and accept the relevant data for
/// each shell command type.
pub trait Builder {
    /// The type which represents the different shell commands.
    type Command;
    /// The type which represents shell words, which can be command names or arguments.
    type Word;
    /// The type which represents a file descriptor redirection.
    type Redirect;

    /// Invoked once a complete command is found. That is, a command delimited by a
    /// newline, semicolon, ampersand, or the end of input.
    ///
    /// # Arguments
    /// * pre_cmd_comments: any comments that appear before the start of the command
    /// * cmd: the command itself, previously generated by the same builder
    /// * separator: indicates how the command was delimited
    /// * post_cmd_comments: any comments that appear after the end of the command
    fn complete_command(&mut self,
                        pre_cmd_comments: Vec<Newline>,
                        cmd: Self::Command,
                        separator: SeparatorKind,
                        pos_cmd_comments: Vec<Newline>)
        -> Result<Self::Command>;

    /// Invoked once two pipeline commands are parsed, which are separated by '&&' or '||'.
    /// Typically the second command is run based on the exit status of the first, running
    /// if the first succeeds for an AND command, or if the first fails for an OR command.
    ///
    /// # Arguments
    /// * first: the command on the left side of the separator
    /// * kind: the type of command parsed, AND or OR
    /// * post_separator_comments: comments appearing between the AND/OR separator and the
    /// start of the second command
    /// * second: the command on the right side of the separator
    fn and_or(&mut self,
              first: Self::Command,
              kind: AndOrKind,
              post_separator_comments: Vec<Newline>,
              second: Self::Command)
        -> Result<Self::Command>;

    /// Invoked when a pipeline of commands is parsed.
    /// A pipeline is one or more commands where the standard output of the previous
    /// typically becomes the standard input of the next.
    ///
    /// # Arguments
    /// * bang: the presence of a `!` at the start of the pipeline, typically indicating
    /// that the pipeline's exit status should be logically inverted.
    /// * cmds: a collection of tuples which are any comments appearing after a pipe token, followed
    /// by the command itself, all in the order they were parsed
    fn pipeline(&mut self,
                bang: bool,
                cmds: Vec<(Vec<Newline>, Self::Command)>)
        -> Result<Self::Command>;

    /// Invoked when the "simplest" possible command is parsed: an executable with arguments.
    ///
    /// # Arguments
    /// * env_vars: environment variables to be defined only for the command before it is run.
    /// * cmd: a tuple of the name of the command to be run and any arguments. This value is
    /// optional since the shell grammar permits that a simple command be made up of only env
    /// var definitions or redirects (or both).
    /// * redirects: redirection of any file descriptors to/from other file descriptors or files.
    fn simple_command(&mut self,
                      env_vars: Vec<(String, Option<Self::Word>)>,
                      cmd: Option<(Self::Word, Vec<Self::Word>)>,
                      redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>;

    /// Invoked when a non-zero number of commands were parsed between balanced curly braces.
    /// Typically these commands should run within the current shell environment.
    ///
    /// # Arguments
    /// * cmds: the commands that were parsed between braces
    /// * redirects: any redirects to be applied over the **entire** group of commands
    fn brace_group(&mut self,
                   cmds: Vec<Self::Command>,
                   redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>;

    /// Invoked when a non-zero number of commands were parsed between balanced parentheses.
    /// Typically these commands should run within their own environment without affecting
    /// the shell's global environment.
    ///
    /// # Arguments
    /// * cmds: the commands that were parsed between parens
    /// * redirects: any redirects to be applied over the **entire** group of commands
    fn subshell(&mut self,
                cmds: Vec<Self::Command>,
                redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>;

    /// Invoked when a loop command like `while` or `until` is parsed.
    /// Typically these commands will execute their body based on the exit status of their guard.
    ///
    /// # Arguments
    /// * kind: the type of the loop: `while` or `until`
    /// * guard: commands that determine how long the loop will run for
    /// * body: commands to be run every iteration of the loop
    /// * redirects: any redirects to be applied over **all** commands part of the loop
    fn loop_command(&mut self,
                    kind: LoopKind,
                    guard_body_pair: GuardBodyPair<Self::Command>,
                    redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>;

    /// Invoked when an `if` conditional command is parsed.
    /// Typically an `if` command is made up of one or more guard-body pairs, where the body
    /// of the first successful corresponding guard is executed. There can also be an optional
    /// `else` part to be run if no guard is successful.
    ///
    /// # Arguments
    /// * fragments: parsed fragments relating to a shell `if` command.
    /// * redirects: any redirects to be applied over **all** commands within the `if` command
    fn if_command(&mut self,
                  fragments: IfFragments<Self::Command>,
                  redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>;

    /// Invoked when a `for` command is parsed.
    /// Typically a `for` command binds a variable to each member in a group of words and
    /// invokes its body with that variable present in the environment. If no words are
    /// specified, the command will iterate over the arguments to the script or enclosing function.
    ///
    /// # Arguments
    /// * fragments: parsed fragments relating to a shell `for` command.
    /// * redirects: any redirects to be applied over **all** commands within the `for` command
    fn for_command(&mut self,
                   fragments: ForFragments<Self::Word, Self::Command>,
                   redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>;

    /// Invoked when a `case` command is parsed.
    /// Typically this command will execute certain commands when a given word matches a pattern.
    ///
    /// # Arguments
    /// * fragments: parsed fragments relating to a shell `case` command.
    /// * redirects: any redirects to be applied over **all** commands part of the `case` block
    fn case_command(&mut self,
                    fragments: CaseFragments<Self::Word, Self::Command>,
                    redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>;

    /// Invoked when a function declaration is parsed.
    /// Typically a function declaration overwrites any previously defined function
    /// within the current environment.
    ///
    /// # Arguments
    /// * name: the name of the function to be created
    /// * body: commands to be run when the function is invoked
    fn function_declaration(&mut self,
                            name: String,
                            body: Self::Command)
        -> Result<Self::Command>;

    /// Invoked when only comments are parsed with no commands following.
    /// This can occur if an entire shell script is commented out or if there
    /// are comments present at the end of the script.
    ///
    /// # Arguments
    /// * comments: the parsed comments
    fn comments(&mut self,
                comments: Vec<Newline>)
        -> Result<()>;

    /// Invoked when a word is parsed.
    ///
    /// # Arguments
    /// * kind: the type of word that was parsed
    fn word(&mut self,
            kind: ComplexWordKind<Self::Command>)
        -> Result<Self::Word>;

    /// Invoked when a redirect is parsed.
    ///
    /// # Arguments
    /// * kind: the type of redirect that was parsed
    fn redirect(&mut self,
                kind: RedirectKind<Self::Word>)
        -> Result<Self::Redirect>;
}

impl Builder for DefaultBuilder {
    type Command  = Command<Self::Word>;
    type Word     = TopLevelWord;
    type Redirect = Redirect<Self::Word>;

    /// Constructs a `Command::Job` node with the provided inputs if the command
    /// was delimited by an ampersand or the command itself otherwise.
    fn complete_command(&mut self,
                        _pre_cmd_comments: Vec<Newline>,
                        cmd: Self::Command,
                        separator: SeparatorKind,
                        _pos_cmd_comments: Vec<Newline>)
        -> Result<Self::Command>
    {
        match separator {
            SeparatorKind::Semi  |
            SeparatorKind::Other |
            SeparatorKind::Newline(_) => Ok(cmd),
            SeparatorKind::Amp => Ok(Command::Job(Box::new(cmd))),
        }
    }

    /// Constructs a `Command::And` or `Command::Or` node with the provided inputs.
    fn and_or(&mut self,
              first: Self::Command,
              kind: AndOrKind,
              _post_separator_comments: Vec<Newline>,
              second: Self::Command)
        -> Result<Self::Command>
    {
        match kind {
            AndOrKind::And => Ok(Command::And(Box::new(first), Box::new(second))),
            AndOrKind::Or  => Ok(Command::Or(Box::new(first), Box::new(second))),
        }
    }

    /// Constructs a `Command::Pipe` node with the provided inputs or a `Command::Simple`
    /// node if only a single command with no status inversion is supplied.
    fn pipeline(&mut self,
                bang: bool,
                cmds: Vec<(Vec<Newline>, Self::Command)>)
        -> Result<Self::Command>
    {
        debug_assert_eq!(cmds.is_empty(), false);
        let mut cmds: Vec<Self::Command> = cmds.into_iter().map(|(_, c)| c).collect();

        // Command::Pipe is the only AST node which allows for a status
        // negation, so we are forced to use it even if we have a single
        // command. Otherwise there is no need to wrap it further.
        if bang || cmds.len() > 1 {
            cmds.shrink_to_fit();
            Ok(Command::Pipe(bang, cmds))
        } else {
            Ok(cmds.pop().unwrap())
        }
    }

    /// Constructs a `Command::Simple` node with the provided inputs.
    fn simple_command(&mut self,
                      mut env_vars: Vec<(String, Option<Self::Word>)>,
                      mut cmd: Option<(Self::Word, Vec<Self::Word>)>,
                      mut redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        env_vars.shrink_to_fit();
        redirects.shrink_to_fit();

        if let Some(&mut (_, ref mut args)) = cmd.as_mut() {
            args.shrink_to_fit();
        }

        Ok(Command::Simple(Box::new(SimpleCommand {
            cmd: cmd,
            vars: env_vars,
            io: redirects,
        })))
    }

    /// Constructs a `Command::Compound(Brace)` node with the provided inputs.
    fn brace_group(&mut self,
                   mut cmds: Vec<Self::Command>,
                   mut redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        cmds.shrink_to_fit();
        redirects.shrink_to_fit();
        Ok(Command::Compound(Box::new(CompoundCommand::Brace(cmds)), redirects))
    }

    /// Constructs a `Command::Compound(Subshell)` node with the provided inputs.
    fn subshell(&mut self,
                mut cmds: Vec<Self::Command>,
                mut redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        cmds.shrink_to_fit();
        redirects.shrink_to_fit();
        Ok(Command::Compound(Box::new(CompoundCommand::Subshell(cmds)), redirects))
    }

    /// Constructs a `Command::Compound(Loop)` node with the provided inputs.
    fn loop_command(&mut self,
                    kind: LoopKind,
                    mut guard_body_pair: GuardBodyPair<Self::Command>,
                    mut redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        guard_body_pair.guard.shrink_to_fit();
        guard_body_pair.body.shrink_to_fit();
        redirects.shrink_to_fit();

        let loop_cmd = match kind {
            LoopKind::While => CompoundCommand::While(guard_body_pair),
            LoopKind::Until => CompoundCommand::Until(guard_body_pair),
        };

        Ok(Command::Compound(Box::new(loop_cmd), redirects))
    }

    /// Constructs a `Command::Compound(If)` node with the provided inputs.
    fn if_command(&mut self,
                  fragments: IfFragments<Self::Command>,
                  mut redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        let IfFragments { mut conditionals, mut else_part } = fragments;

        for guard_body_pair in &mut conditionals {
            guard_body_pair.guard.shrink_to_fit();
            guard_body_pair.body.shrink_to_fit();
        }

        for els in &mut else_part {
            els.shrink_to_fit();
        }

        redirects.shrink_to_fit();

        Ok(Command::Compound(Box::new(CompoundCommand::If(conditionals, else_part)), redirects))
    }

    /// Constructs a `Command::Compound(For)` node with the provided inputs.
    fn for_command(&mut self,
                   mut fragments: ForFragments<Self::Word, Self::Command>,
                   mut redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        for word in &mut fragments.words {
            word.shrink_to_fit();
        }

        fragments.var.shrink_to_fit();
        fragments.body.shrink_to_fit();
        redirects.shrink_to_fit();
        Ok(Command::Compound(
            Box::new(CompoundCommand::For(fragments.var, fragments.words, fragments.body)),
            redirects
        ))
    }

    /// Constructs a `Command::Compound(Case)` node with the provided inputs.
    fn case_command(&mut self,
                    fragments: CaseFragments<Self::Word, Self::Command>,
                    mut redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        let arms = fragments.arms.into_iter().map(|(pat_fragment, mut body)| {
            let mut patterns = pat_fragment.pattern_alternatives;
            patterns.shrink_to_fit();
            body.shrink_to_fit();
            (patterns, body)
        }).collect();

        redirects.shrink_to_fit();
        Ok(Command::Compound(Box::new(CompoundCommand::Case(fragments.word, arms)), redirects))
    }

    /// Constructs a `Command::Function` node with the provided inputs.
    fn function_declaration(&mut self,
                            name: String,
                            body: Self::Command)
        -> Result<Self::Command>
    {
        Ok(Command::Function(name, Rc::new(body)))
    }

    /// Ignored by the builder.
    fn comments(&mut self, _comments: Vec<Newline>) -> Result<()> {
        Ok(())
    }

    /// Constructs a `ast::Word` from the provided input.
    fn word(&mut self, kind: ComplexWordKind<Self::Command>) -> Result<Self::Word> {
        use self::ParameterSubstitutionKind::*;

        macro_rules! map {
            ($pat:expr) => {
                match $pat {
                    Some(w) => Some(try!(self.word(*w))),
                    None => None,
                }
            }
        }

        let mut map_simple = |kind| {
            let simple = match kind {
                SimpleWordKind::Literal(s)      => SimpleWord::Literal(s),
                SimpleWordKind::Escaped(s)      => SimpleWord::Escaped(s),
                SimpleWordKind::Param(p)        => SimpleWord::Param(p),
                SimpleWordKind::Star            => SimpleWord::Star,
                SimpleWordKind::Question        => SimpleWord::Question,
                SimpleWordKind::SquareOpen      => SimpleWord::SquareOpen,
                SimpleWordKind::SquareClose     => SimpleWord::SquareClose,
                SimpleWordKind::Tilde           => SimpleWord::Tilde,
                SimpleWordKind::Colon           => SimpleWord::Colon,

                SimpleWordKind::CommandSubst(c) => SimpleWord::Subst(
                    Box::new(ParameterSubstitution::Command(c))
                ),

                SimpleWordKind::Subst(s) => SimpleWord::Subst(Box::new(match s {
                    Len(p)                     => ParameterSubstitution::Len(p),
                    Command(c)                 => ParameterSubstitution::Command(c),
                    Arith(a)                   => ParameterSubstitution::Arith(a),
                    Default(c, p, w)           => ParameterSubstitution::Default(c, p, map!(w)),
                    Assign(c, p, w)            => ParameterSubstitution::Assign(c, p, map!(w)),
                    Error(c, p, w)             => ParameterSubstitution::Error(c, p, map!(w)),
                    Alternative(c, p, w)       => ParameterSubstitution::Alternative(c, p, map!(w)),
                    RemoveSmallestSuffix(p, w) => ParameterSubstitution::RemoveSmallestSuffix(p, map!(w)),
                    RemoveLargestSuffix(p, w)  => ParameterSubstitution::RemoveLargestSuffix(p, map!(w)),
                    RemoveSmallestPrefix(p, w) => ParameterSubstitution::RemoveSmallestPrefix(p, map!(w)),
                    RemoveLargestPrefix(p, w)  => ParameterSubstitution::RemoveLargestPrefix(p, map!(w)),
                })),
            };
            Ok(simple)
        };

        let mut map_word = |kind| {
            let word = match kind {
                WordKind::Simple(s)       => Word::Simple(Box::new(try!(map_simple(s)))),
                WordKind::SingleQuoted(s) => Word::SingleQuoted(s),
                WordKind::DoubleQuoted(v) => Word::DoubleQuoted(try!(
                    v.into_iter()
                     .map(&mut map_simple)
                     .collect::<Result<Vec<SimpleWord<Self::Word, Self::Command>>>>()
                )),
            };
            Ok(word)
        };

        let word = match compress(kind) {
            ComplexWordKind::Single(s)     => ComplexWord::Single(try!(map_word(s))),
            ComplexWordKind::Concat(words) => ComplexWord::Concat(try!(
                    words.into_iter()
                         .map(map_word)
                         .collect::<Result<Vec<Word<Self::Word, Self::Command>>>>()
            )),
        };

        Ok(TopLevelWord(word))
    }

    /// Constructs a `ast::Redirect` from the provided input.
    fn redirect(&mut self,
                kind: RedirectKind<Self::Word>)
        -> Result<Self::Redirect>
    {
        let io = match kind {
            RedirectKind::Read(fd, path)      => Redirect::Read(fd, path),
            RedirectKind::Write(fd, path)     => Redirect::Write(fd, path),
            RedirectKind::ReadWrite(fd, path) => Redirect::ReadWrite(fd, path),
            RedirectKind::Append(fd, path)    => Redirect::Append(fd, path),
            RedirectKind::Clobber(fd, path)   => Redirect::Clobber(fd, path),
            RedirectKind::Heredoc(fd, body)   => Redirect::Heredoc(fd, body),
            RedirectKind::DupRead(src, dst)   => Redirect::DupRead(src, dst),
            RedirectKind::DupWrite(src, dst)  => Redirect::DupWrite(src, dst),
        };

        Ok(io)
    }
}

impl<'a, T: Builder + ?Sized> Builder for &'a mut T {
    type Command = T::Command;
    type Word = T::Word;
    type Redirect = T::Redirect;

    fn complete_command(&mut self,
                        pre_cmd_comments: Vec<Newline>,
                        cmd: Self::Command,
                        separator: SeparatorKind,
                        post_cmd_comments: Vec<Newline>)
        -> Result<Self::Command>
    {
        (**self).complete_command(pre_cmd_comments, cmd, separator, post_cmd_comments)
    }

    fn and_or(&mut self,
              first: Self::Command,
              kind: AndOrKind,
              post_separator_comments: Vec<Newline>,
              second: Self::Command)
        -> Result<Self::Command>
    {
        (**self).and_or(first, kind, post_separator_comments, second)
    }

    fn pipeline(&mut self,
                bang: bool,
                cmds: Vec<(Vec<Newline>, Self::Command)>)
        -> Result<Self::Command>
    {
        (**self).pipeline(bang, cmds)
    }

    fn simple_command(&mut self,
                      env_vars: Vec<(String, Option<Self::Word>)>,
                      cmd: Option<(Self::Word, Vec<Self::Word>)>,
                      redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        (**self).simple_command(env_vars, cmd, redirects)
    }

    fn brace_group(&mut self,
                   cmds: Vec<Self::Command>,
                   redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        (**self).brace_group(cmds, redirects)
    }

    fn subshell(&mut self,
                cmds: Vec<Self::Command>,
                redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        (**self).subshell(cmds, redirects)
    }

    fn loop_command(&mut self,
                    kind: LoopKind,
                    guard_body_pair: GuardBodyPair<Self::Command>,
                    redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        (**self).loop_command(kind, guard_body_pair, redirects)
    }

    fn if_command(&mut self,
                  fragments: IfFragments<Self::Command>,
                  redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        (**self).if_command(fragments, redirects)
    }

    fn for_command(&mut self,
                   fragments: ForFragments<Self::Word, Self::Command>,
                   redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        (**self).for_command(fragments, redirects)
    }

    fn case_command(&mut self,
                    fragments: CaseFragments<Self::Word, Self::Command>,
                    redirects: Vec<Self::Redirect>)
        -> Result<Self::Command>
    {
        (**self).case_command(fragments, redirects)
    }

    fn function_declaration(&mut self,
                            name: String,
                            body: Self::Command)
        -> Result<Self::Command>
    {
        (**self).function_declaration(name, body)
    }

    fn comments(&mut self,
                comments: Vec<Newline>)
        -> Result<()>
    {
        (**self).comments(comments)
    }

    fn word(&mut self,
            kind: ComplexWordKind<Self::Command>)
        -> Result<Self::Word>
    {
        (**self).word(kind)
    }

    fn redirect(&mut self,
                kind: RedirectKind<Self::Word>)
        -> Result<Self::Redirect>
    {
        (**self).redirect(kind)
    }
}

/// A `Builder` implementation which builds shell commands using the AST definitions in the `ast` module.
#[derive(Debug, Copy, Clone)]
pub struct DefaultBuilder;

impl ::std::default::Default for DefaultBuilder {
    fn default() -> DefaultBuilder {
        DefaultBuilder
    }
}

struct Coalesce<I: Iterator, F> {
    iter: I,
    cur: Option<I::Item>,
    func: F,
}

impl<I: Iterator, F> Coalesce<I, F> {
    fn new(iter: I, func: F) -> Self {
        Coalesce {
            iter: iter,
            cur: None,
            func: func,
        }
    }
}

type CoalesceResult<T> = ::std::result::Result<T, (T, T)>;
impl<I, F> Iterator for Coalesce<I, F>
    where I: Iterator,
          F: FnMut(I::Item, I::Item) -> CoalesceResult<I::Item>
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.cur.take().or_else(|| self.iter.next());
        let (mut left, mut right) = match (cur, self.iter.next()) {
            (Some(l), Some(r)) => (l, r),
            (Some(l), None) |
            (None, Some(l)) => return Some(l),
            (None, None) => return None,
        };

        loop {
            match (self.func)(left, right) {
                Ok(combined) => match self.iter.next() {
                    Some(next) => {
                        left = combined;
                        right = next;
                    },
                    None => return Some(combined),
                },

                Err((left, right)) => {
                    debug_assert!(self.cur.is_none());
                    self.cur = Some(right);
                    return Some(left);
                },
            }
        }
    }
}

fn compress<C>(word: ComplexWordKind<C>) -> ComplexWordKind<C> {
    use self::ComplexWordKind::*;
    use self::SimpleWordKind::*;
    use self::WordKind::*;

    fn coalesce_simple<C>(a: SimpleWordKind<C>, b: SimpleWordKind<C>)
        -> CoalesceResult<SimpleWordKind<C>>
    {
        match (a, b) {
            (Literal(mut a), Literal(b)) => {
                a.push_str(&b);
                Ok(Literal(a))
            },
            (a, b) => Err((a, b)),
        }
    }

    fn coalesce_word<C>(a: WordKind<C>, b: WordKind<C>) -> CoalesceResult<WordKind<C>> {
        match (a, b) {
            (Simple(a), Simple(b)) => coalesce_simple(a, b).map(Simple)
                                                           .map_err(|(a, b)| (Simple(a), Simple(b))),
            (SingleQuoted(mut a), SingleQuoted(b)) => {
                a.push_str(&b);
                Ok(SingleQuoted(a))
            },
            (DoubleQuoted(a), DoubleQuoted(b)) => {
                let quoted = Coalesce::new(a.into_iter().chain(b), coalesce_simple).collect();
                Ok(DoubleQuoted(quoted))
            },
            (a, b) => Err((a, b)),
        }
    }

    match word {
        Single(s) => Single(match s {
            s@Simple(_) |
            s@SingleQuoted(_) => s,
            DoubleQuoted(v) => DoubleQuoted(Coalesce::new(v.into_iter(), coalesce_simple).collect()),
        }),
        Concat(v) => {
            let mut body: Vec<_> = Coalesce::new(v.into_iter(), coalesce_word).collect();
            if body.len() == 1 {
                Single(body.pop().unwrap())
            } else {
                Concat(body)
            }
        }
    }
}
