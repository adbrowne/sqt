/// Parser implementation with error recovery
use crate::lexer::{tokenize, Token};
use crate::syntax_kind::{SmeltLanguage, SyntaxKind};
use crate::SyntaxKind::*;
use rowan::{GreenNode, GreenNodeBuilder, TextRange};

/// Result of parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    pub green_node: GreenNode,
    pub errors: Vec<ParseError>,
}

impl Parse {
    pub fn syntax(&self) -> rowan::SyntaxNode<SmeltLanguage> {
        rowan::SyntaxNode::new_root(self.green_node.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub range: TextRange,
}

/// Parse input text into a CST
pub fn parse(input: &str) -> Parse {
    let tokens = tokenize(input);
    let mut parser = Parser::new(input, &tokens);
    parser.parse_file();
    parser.finish()
}

struct Parser<'a> {
    input: &'a str,
    tokens: &'a [Token],
    pos: usize,
    offset: usize,
    builder: GreenNodeBuilder<'static>,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, tokens: &'a [Token]) -> Self {
        Self {
            input,
            tokens,
            pos: 0,
            offset: 0,
            builder: GreenNodeBuilder::new(),
            errors: Vec::new(),
        }
    }

    fn finish(self) -> Parse {
        Parse {
            green_node: self.builder.finish(),
            errors: self.errors,
        }
    }

    /// Current token kind
    fn current(&self) -> SyntaxKind {
        self.tokens.get(self.pos).map(|t| t.kind).unwrap_or(EOF)
    }

    /// Check if at specific token kind
    fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    /// Check if at any of the given kinds
    fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.current())
    }

    /// Advance to next token, consuming trivia
    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            let token = self.tokens[self.pos];
            let text = &self.input[self.offset..self.offset + token.len];
            self.builder.token(token.kind.into(), text);
            self.offset += token.len;
            self.pos += 1;
        }
    }

    /// Skip trivia (whitespace, comments)
    fn skip_trivia(&mut self) {
        while self.current().is_trivia() {
            self.advance();
        }
    }

    /// Expect a specific token kind, report error if not present
    fn expect(&mut self, kind: SyntaxKind) -> bool {
        self.skip_trivia();
        if self.at(kind) {
            self.advance();
            true
        } else {
            self.error(format!("Expected {:?}, found {:?}", kind, self.current()));
            false
        }
    }

    /// Start a composite node
    fn start_node(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind.into());
    }

    /// Start a composite node at a checkpoint (for lookahead/backtracking)
    fn start_node_at(&mut self, checkpoint: rowan::Checkpoint, kind: SyntaxKind) {
        self.builder.start_node_at(checkpoint, kind.into());
    }

    /// Finish current node
    fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    /// Report a parse error
    fn error(&mut self, message: String) {
        let start = self.offset as u32;
        let end = (self.offset + self.tokens.get(self.pos).map(|t| t.len).unwrap_or(0)) as u32;
        self.errors.push(ParseError {
            message,
            range: TextRange::new(start.into(), end.into()),
        });
    }

    /// Synchronize to one of the given tokens (error recovery)
    fn sync_to(&mut self, kinds: &[SyntaxKind]) {
        while !self.at(EOF) && !self.at_any(kinds) {
            self.start_node(ERROR);
            self.advance();
            self.finish_node();
        }
    }

    /// Check if current token is a keyword that would end a table reference
    fn at_keyword_that_ends_table_ref(&self) -> bool {
        // Keywords that can follow a table reference in the FROM clause
        self.at_any(&[
            WHERE_KW,
            GROUP_KW,
            // JOIN keywords
            JOIN_KW,
            INNER_KW,
            LEFT_KW,
            RIGHT_KW,
            FULL_KW,
            CROSS_KW,
        ])
    }

    /// Check if current token can start an expression
    fn at_expression_start(&self) -> bool {
        self.at_any(&[IDENT, NUMBER, STRING, LPAREN, NOT_KW, CASE_KW, CAST_KW, EXISTS_KW])
    }

    // ===== Parsing rules =====

    fn parse_file(&mut self) {
        self.start_node(FILE);

        self.skip_trivia();

        // Parse SELECT statement
        if self.at(SELECT_KW) {
            self.parse_select_stmt();
        } else if !self.at(EOF) {
            self.error("Expected SELECT statement".to_string());
            self.sync_to(&[EOF]);
        }

        // Consume remaining trivia
        while !self.at(EOF) {
            self.advance();
        }

        self.finish_node();
    }

    fn parse_select_stmt(&mut self) {
        self.start_node(SELECT_STMT);

        // SELECT
        self.expect(SELECT_KW);

        // Select list
        self.parse_select_list();

        // FROM clause
        self.skip_trivia();
        if self.at(FROM_KW) {
            self.parse_from_clause();
        }

        // WHERE clause
        self.skip_trivia();
        if self.at(WHERE_KW) {
            self.parse_where_clause();
        }

        // GROUP BY clause
        self.skip_trivia();
        if self.at(GROUP_KW) {
            self.parse_group_by_clause();
        }

        self.finish_node();
    }

    fn parse_select_list(&mut self) {
        self.start_node(SELECT_LIST);
        self.skip_trivia();

        // Handle SELECT *
        if self.at(STAR) {
            self.start_node(SELECT_ITEM);
            self.advance();
            self.finish_node();
        } else {
            // Parse comma-separated select items
            loop {
                self.parse_select_item();

                self.skip_trivia();
                if self.at(COMMA) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.finish_node();
    }

    fn parse_select_item(&mut self) {
        self.start_node(SELECT_ITEM);
        self.skip_trivia();

        // Parse expression
        self.parse_expression();

        // Optional AS alias
        self.skip_trivia();
        if self.at(AS_KW) {
            self.advance();
            self.skip_trivia();
            if self.at(IDENT) {
                self.advance();
            }
        } else if self.at(IDENT) {
            // Implicit alias (no AS keyword)
            self.advance();
        }

        self.finish_node();
    }

    fn parse_from_clause(&mut self) {
        self.start_node(FROM_CLAUSE);

        self.expect(FROM_KW);

        // Parse first table reference (required)
        self.parse_table_ref();

        // Parse zero or more JOIN clauses
        loop {
            self.skip_trivia();
            if self.at_any(&[JOIN_KW, INNER_KW, LEFT_KW, RIGHT_KW, FULL_KW, CROSS_KW]) {
                self.parse_join_clause();
            } else {
                break;
            }
        }

        self.finish_node();
    }

    fn parse_table_ref(&mut self) {
        self.start_node(TABLE_REF);
        self.skip_trivia();

        if self.at(LPAREN) {
            // Could be a subquery
            let checkpoint = self.builder.checkpoint();
            self.advance(); // consume LPAREN
            self.skip_trivia();

            // Check if it's a subquery (starts with SELECT)
            if self.at(SELECT_KW) {
                self.start_node_at(checkpoint, SUBQUERY);
                self.parse_select_stmt();
                self.skip_trivia();
                self.expect(RPAREN);
                self.finish_node(); // Close SUBQUERY
            } else {
                // Not a subquery, error
                self.error("Expected SELECT in subquery".to_string());
                self.expect(RPAREN);
            }
        } else if self.at(IDENT) {
            // Use builder checkpoint for proper lookahead
            let checkpoint = self.builder.checkpoint();
            self.advance(); // Consume IDENT
            self.skip_trivia();

            if self.at(LPAREN) {
                // It's a simple function call - wrap in FUNCTION_CALL node using checkpoint
                self.start_node_at(checkpoint, FUNCTION_CALL);
                self.parse_arg_list();
                self.finish_node(); // Close FUNCTION_CALL
            } else if self.at(DOT) {
                // Could be schema.table or namespace.func()
                self.advance(); // Consume DOT
                self.skip_trivia();
                self.expect(IDENT); // Consume second IDENT
                self.skip_trivia();

                if self.at(LPAREN) {
                    // Namespaced function call: smelt.ref()
                    self.start_node_at(checkpoint, FUNCTION_CALL);
                    self.parse_arg_list();
                    self.finish_node(); // Close FUNCTION_CALL
                }
                // else: just a qualified table name (schema.table), already consumed
            }
            // else: simple identifier, already consumed
        } else {
            self.error("Expected table reference".to_string());
        }

        // Optional AS alias (explicit with AS keyword or implicit)
        self.skip_trivia();
        if self.at(AS_KW) {
            self.advance();
            self.skip_trivia();
            self.expect(IDENT);
        } else if self.at(IDENT) && !self.at_keyword_that_ends_table_ref() {
            // Implicit alias (no AS keyword)
            // Only consume if it's not a keyword that would end the table ref
            self.advance();
        }

        self.finish_node();
    }

    #[allow(clippy::if_same_then_else)]
    fn parse_join_clause(&mut self) {
        self.start_node(JOIN_CLAUSE);

        // Parse JOIN type modifiers (INNER, LEFT, RIGHT, FULL OUTER, CROSS)
        // Note: The if-else blocks are intentionally similar for clarity
        if self.at(INNER_KW) {
            self.advance();
            self.skip_trivia();
        } else if self.at(LEFT_KW) {
            self.advance();
            self.skip_trivia();
            if self.at(OUTER_KW) {
                self.advance();
                self.skip_trivia();
            }
        } else if self.at(RIGHT_KW) {
            self.advance();
            self.skip_trivia();
            if self.at(OUTER_KW) {
                self.advance();
                self.skip_trivia();
            }
        } else if self.at(FULL_KW) {
            self.advance();
            self.skip_trivia();
            if self.at(OUTER_KW) {
                self.advance();
                self.skip_trivia();
            }
        } else if self.at(CROSS_KW) {
            self.advance();
            self.skip_trivia();
        }
        // Note: Bare JOIN defaults to INNER JOIN

        // Expect JOIN keyword
        if !self.expect(JOIN_KW) {
            // Error recovery: missing JOIN keyword
            self.error("Expected JOIN keyword".to_string());
            self.finish_node();
            return;
        }

        // Parse table reference
        self.skip_trivia();
        if !self.at(IDENT) {
            // Error recovery: missing table reference
            self.error("Expected table reference after JOIN".to_string());
            self.finish_node();
            return;
        }
        self.parse_table_ref();

        // Parse join condition (ON or USING)
        // CROSS JOIN doesn't require a condition
        self.skip_trivia();
        if self.at(ON_KW) || self.at(USING_KW) {
            self.parse_join_condition();
        }

        self.finish_node();
    }

    fn parse_join_condition(&mut self) {
        self.start_node(JOIN_CONDITION);

        if self.at(ON_KW) {
            // ON expression
            self.advance();
            self.skip_trivia();

            if !self.at_expression_start() {
                self.error("Expected expression after ON".to_string());
                self.finish_node();
                return;
            }
            self.parse_expression();

        } else if self.at(USING_KW) {
            // USING (col1, col2, ...)
            self.advance();
            self.skip_trivia();

            if !self.expect(LPAREN) {
                self.error("Expected '(' after USING".to_string());
                self.finish_node();
                return;
            }

            // Parse comma-separated column list
            loop {
                self.skip_trivia();
                if !self.at(IDENT) {
                    self.error("Expected column name in USING clause".to_string());
                    break;
                }
                self.advance();

                self.skip_trivia();
                if self.at(COMMA) {
                    self.advance();
                } else {
                    break;
                }
            }

            self.expect(RPAREN);
        }

        self.finish_node();
    }

    fn parse_where_clause(&mut self) {
        self.start_node(WHERE_CLAUSE);
        self.expect(WHERE_KW);
        self.parse_expression();
        self.finish_node();
    }

    fn parse_group_by_clause(&mut self) {
        self.start_node(GROUP_BY_CLAUSE);
        self.expect(GROUP_KW);
        self.expect(BY_KW);

        // Parse comma-separated column list
        loop {
            self.parse_expression();

            self.skip_trivia();
            if self.at(COMMA) {
                self.advance();
            } else {
                break;
            }
        }

        self.finish_node();
    }

    fn parse_expression(&mut self) {
        self.start_node(EXPRESSION);
        self.skip_trivia();

        self.parse_or_expr();

        self.finish_node();
    }

    fn parse_or_expr(&mut self) {
        self.parse_and_expr();

        while self.at(OR_KW) {
            self.start_node(BINARY_EXPR);
            self.advance();
            self.skip_trivia();
            self.parse_and_expr();
            self.finish_node();
        }
    }

    fn parse_and_expr(&mut self) {
        self.parse_comparison_expr();

        while self.at(AND_KW) {
            self.start_node(BINARY_EXPR);
            self.advance();
            self.skip_trivia();
            self.parse_comparison_expr();
            self.finish_node();
        }
    }

    fn parse_comparison_expr(&mut self) {
        self.parse_additive_expr();

        loop {
            self.skip_trivia();
            if self.at_any(&[EQ, NE, LT, GT, LE, GE]) {
                self.start_node(BINARY_EXPR);
                self.advance();
                self.skip_trivia();
                self.parse_additive_expr();
                self.finish_node();
            } else if self.at(IS_KW) {
                // IS [NOT] NULL
                self.start_node(BINARY_EXPR);
                self.advance(); // consume IS
                self.skip_trivia();
                if self.at(NOT_KW) {
                    self.advance(); // consume NOT
                    self.skip_trivia();
                }
                if self.at(NULL_KW) {
                    self.advance(); // consume NULL
                }
                self.finish_node();
            } else if self.at(BETWEEN_KW) {
                // BETWEEN low AND high
                self.parse_between_expr();
            } else if self.at(IN_KW) {
                // IN (values...)
                self.parse_in_expr();
            } else {
                break;
            }
        }
    }

    fn parse_between_expr(&mut self) {
        self.start_node(BETWEEN_EXPR);
        self.expect(BETWEEN_KW);

        // Parse lower bound
        self.skip_trivia();
        self.parse_additive_expr();

        // Expect AND
        self.skip_trivia();
        if !self.expect(AND_KW) {
            self.error("Expected AND in BETWEEN expression".to_string());
        }

        // Parse upper bound
        self.skip_trivia();
        self.parse_additive_expr();

        self.finish_node();
    }

    fn parse_in_expr(&mut self) {
        self.start_node(IN_EXPR);
        self.expect(IN_KW);

        self.skip_trivia();
        if !self.expect(LPAREN) {
            self.error("Expected '(' after IN".to_string());
            self.finish_node();
            return;
        }

        self.skip_trivia();

        // Check if it's a subquery (starts with SELECT)
        if self.at(SELECT_KW) {
            self.parse_subquery();
        } else {
            // Parse comma-separated value list
            loop {
                self.skip_trivia();
                if self.at(RPAREN) {
                    break;
                }

                self.parse_expression();

                self.skip_trivia();
                if self.at(COMMA) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.expect(RPAREN);
        self.finish_node();
    }

    fn parse_additive_expr(&mut self) {
        self.parse_multiplicative_expr();

        while self.at_any(&[PLUS, MINUS]) {
            self.start_node(BINARY_EXPR);
            self.advance();
            self.skip_trivia();
            self.parse_multiplicative_expr();
            self.finish_node();
        }
    }

    fn parse_multiplicative_expr(&mut self) {
        self.parse_unary_expr();

        while self.at_any(&[STAR, DIVIDE]) {
            self.start_node(BINARY_EXPR);
            self.advance();
            self.skip_trivia();
            self.parse_unary_expr();
            self.finish_node();
        }
    }

    fn parse_unary_expr(&mut self) {
        self.skip_trivia();

        // Handle unary operators (-, NOT)
        if self.at_any(&[MINUS, NOT_KW]) {
            self.start_node(BINARY_EXPR); // Reuse BINARY_EXPR for unary ops
            self.advance(); // consume operator
            self.skip_trivia();
            self.parse_unary_expr(); // Allow chaining: --x
            self.finish_node();
        } else {
            self.parse_primary_expr();
        }
    }

    fn parse_primary_expr(&mut self) {
        self.skip_trivia();

        if self.at(CASE_KW) {
            self.parse_case_expr();
        } else if self.at(CAST_KW) {
            self.parse_cast_expr();
        } else if self.at(EXISTS_KW) {
            self.parse_exists_expr();
        } else if self.at(LPAREN) {
            // Could be: parenthesized expression, subquery, or function call
            let checkpoint = self.builder.checkpoint();
            self.advance(); // consume LPAREN
            self.skip_trivia();

            // Check if it's a subquery (starts with SELECT)
            if self.at(SELECT_KW) {
                self.start_node_at(checkpoint, SUBQUERY);
                self.parse_select_stmt();
                self.skip_trivia();
                self.expect(RPAREN);
                self.finish_node();
            } else {
                // Grouped expression
                self.parse_expression();
                self.skip_trivia();
                self.expect(RPAREN);
            }
        } else if self.at(IDENT) {
            // Could be column reference, qualified name, or function call
            let checkpoint = self.builder.checkpoint();
            self.advance(); // consume first IDENT
            self.skip_trivia();

            if self.at(LPAREN) {
                // Simple function call: func()
                self.start_node_at(checkpoint, FUNCTION_CALL);
                self.parse_arg_list();
                self.finish_node();
            } else if self.at(DOT) {
                // Could be table.column or namespace.func()
                self.advance(); // consume DOT
                self.skip_trivia();
                self.expect(IDENT); // consume second IDENT
                self.skip_trivia();

                if self.at(LPAREN) {
                    // Namespaced function call: smelt.ref()
                    self.start_node_at(checkpoint, FUNCTION_CALL);
                    self.parse_arg_list();
                    self.finish_node();
                }
                // else: just a qualified name (table.column), no extra node needed
            } else if self.at(DOUBLE_COLON) {
                // PostgreSQL cast: expr::type
                self.start_node_at(checkpoint, CAST_EXPR);
                self.advance(); // consume ::
                self.skip_trivia();
                self.parse_type_spec();
                self.finish_node();
            }
            // else: just an identifier, no extra node needed
        } else if self.current().is_literal() || self.at(STAR) {
            self.advance();
        } else {
            self.error(format!("Expected expression, found {:?}", self.current()));
        }
    }

    fn parse_case_expr(&mut self) {
        self.start_node(CASE_EXPR);
        self.expect(CASE_KW);

        self.skip_trivia();

        // Check if it's simple CASE (CASE expr WHEN ...) or searched CASE (CASE WHEN ...)
        // If the next token after CASE is not WHEN, it's a simple CASE
        let is_simple_case = !self.at(WHEN_KW);
        if is_simple_case {
            // Simple CASE - parse the case value expression
            // Use a more restricted parse to avoid consuming the WHEN keyword
            self.parse_additive_expr();
            self.skip_trivia();
        }

        // Parse WHEN clauses
        while self.at(WHEN_KW) {
            self.parse_when_clause();
            self.skip_trivia();
        }

        // Optional ELSE clause
        if self.at(ELSE_KW) {
            self.advance(); // consume ELSE
            self.skip_trivia();
            self.parse_expression();
            self.skip_trivia();
        }

        // Expect END
        if !self.expect(END_KW) {
            self.error("Expected END to close CASE expression".to_string());
        }

        self.finish_node();
    }

    fn parse_when_clause(&mut self) {
        self.start_node(WHEN_CLAUSE);
        self.expect(WHEN_KW);

        // Parse condition or value (depends on simple vs searched CASE)
        // Use comparison_expr to avoid consuming beyond THEN keyword
        self.skip_trivia();
        self.parse_comparison_expr();

        // Expect THEN
        self.skip_trivia();
        if !self.expect(THEN_KW) {
            self.error("Expected THEN in WHEN clause".to_string());
        }

        // Parse result - use comparison_expr to avoid consuming beyond WHEN/ELSE/END
        self.skip_trivia();
        self.parse_comparison_expr();

        self.finish_node();
    }

    fn parse_cast_expr(&mut self) {
        self.start_node(CAST_EXPR);
        self.expect(CAST_KW);

        self.skip_trivia();
        if !self.expect(LPAREN) {
            self.error("Expected '(' after CAST".to_string());
            self.finish_node();
            return;
        }

        // Parse expression to cast
        self.skip_trivia();
        self.parse_expression();

        // Expect AS
        self.skip_trivia();
        if !self.expect(AS_KW) {
            self.error("Expected AS in CAST expression".to_string());
        }

        // Parse type
        self.skip_trivia();
        self.parse_type_spec();

        self.expect(RPAREN);
        self.finish_node();
    }

    fn parse_type_spec(&mut self) {
        self.start_node(TYPE_SPEC);

        // Type name (identifier)
        if !self.at(IDENT) {
            self.error("Expected type name".to_string());
            self.finish_node();
            return;
        }
        self.advance();

        // Optional type parameters: VARCHAR(255), DECIMAL(10,2), etc.
        self.skip_trivia();
        if self.at(LPAREN) {
            self.advance(); // consume LPAREN

            // Parse comma-separated parameters
            loop {
                self.skip_trivia();
                if self.at(RPAREN) {
                    break;
                }

                // Type parameters are typically numbers
                if self.at(NUMBER) {
                    self.advance();
                } else if self.at(IDENT) {
                    // Some types might have identifier parameters
                    self.advance();
                } else {
                    self.error("Expected type parameter".to_string());
                    break;
                }

                self.skip_trivia();
                if self.at(COMMA) {
                    self.advance();
                } else {
                    break;
                }
            }

            self.expect(RPAREN);
        }

        self.finish_node();
    }

    fn parse_subquery(&mut self) {
        self.start_node(SUBQUERY);
        self.parse_select_stmt();
        self.finish_node();
    }

    fn parse_exists_expr(&mut self) {
        self.start_node(EXISTS_EXPR);
        self.expect(EXISTS_KW);

        self.skip_trivia();
        if !self.expect(LPAREN) {
            self.error("Expected '(' after EXISTS".to_string());
            self.finish_node();
            return;
        }

        self.skip_trivia();
        if self.at(SELECT_KW) {
            self.parse_subquery();
        } else {
            self.error("Expected SELECT after EXISTS (".to_string());
        }

        self.expect(RPAREN);
        self.finish_node();
    }

    fn parse_arg_list(&mut self) {
        self.start_node(ARG_LIST);
        self.expect(LPAREN);
        self.skip_trivia();

        if !self.at(RPAREN) {
            loop {
                self.parse_argument();

                self.skip_trivia();
                if self.at(COMMA) {
                    self.advance();
                    self.skip_trivia();
                } else {
                    break;
                }
            }
        }

        self.expect(RPAREN);
        self.finish_node();
    }

    fn parse_argument(&mut self) {
        self.skip_trivia();

        // Check for named parameter: IDENT => expression
        if self.at(IDENT) {
            // Look ahead to check for ARROW
            let checkpoint = self.builder.checkpoint();
            self.advance(); // consume IDENT
            self.skip_trivia();

            if self.at(ARROW) {
                // It's a named parameter
                self.start_node_at(checkpoint, NAMED_PARAM);
                self.advance(); // consume ARROW
                self.skip_trivia();
                self.parse_expression();
                self.finish_node();
            } else {
                // Not a named parameter, need to parse the rest as expression
                // The IDENT is already consumed, continue parsing expression from here
                self.skip_trivia();

                // Handle cases where IDENT might be followed by operators or function calls
                if self.at(LPAREN) {
                    // Function call - wrap in FUNCTION_CALL
                    self.start_node_at(checkpoint, FUNCTION_CALL);
                    self.parse_arg_list();
                    self.finish_node();
                } else if self.at(DOT) {
                    // Qualified name or namespaced function
                    self.advance();
                    self.skip_trivia();
                    self.expect(IDENT);
                    self.skip_trivia();

                    if self.at(LPAREN) {
                        // Namespaced function call
                        self.start_node_at(checkpoint, FUNCTION_CALL);
                        self.parse_arg_list();
                        self.finish_node();
                    }
                }
                // Otherwise, the IDENT alone is the expression (already consumed)
            }
        } else {
            // Not starting with IDENT, parse as regular expression
            self.parse_expression();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inner_join() {
        let input = "SELECT * FROM users INNER JOIN orders ON users.id = orders.user_id";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_left_join() {
        let input = "SELECT * FROM users LEFT JOIN orders ON users.id = orders.user_id";
        let parse = parse(input);
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_right_join() {
        let input = "SELECT * FROM users RIGHT JOIN orders ON users.id = orders.user_id";
        let parse = parse(input);
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_full_join() {
        let input = "SELECT * FROM users FULL JOIN orders ON users.id = orders.user_id";
        let parse = parse(input);
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_cross_join() {
        let input = "SELECT * FROM users CROSS JOIN countries";
        let parse = parse(input);
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_multiple_joins() {
        let input = "SELECT * FROM users
                     INNER JOIN orders ON users.id = orders.user_id
                     LEFT JOIN products ON orders.product_id = products.id";
        let parse = parse(input);
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_using_clause() {
        let input = "SELECT * FROM users JOIN orders USING (user_id)";
        let parse = parse(input);
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_join_error_recovery_missing_table() {
        let input = "SELECT * FROM users JOIN";
        let parse = parse(input);
        assert!(!parse.errors.is_empty());
        assert!(parse.errors[0].message.contains("table"));
    }

    #[test]
    fn test_join_error_recovery_missing_on() {
        let input = "SELECT * FROM users JOIN orders ON";
        let parse = parse(input);
        assert!(!parse.errors.is_empty());
        assert!(parse.errors[0].message.contains("expression"));
    }

    // Phase 10: Expression Enhancement Tests

    #[test]
    fn test_case_searched() {
        let input = "SELECT CASE WHEN status = 'active' THEN 1 WHEN status = 'pending' THEN 0 ELSE -1 END FROM users";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_case_simple() {
        let input = "SELECT CASE status WHEN 'active' THEN 1 WHEN 'pending' THEN 0 ELSE -1 END FROM users";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_case_no_else() {
        let input = "SELECT CASE WHEN status = 'active' THEN 1 END FROM users";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_cast_standard() {
        let input = "SELECT CAST(price AS INTEGER) FROM products";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_cast_postgres_double_colon() {
        let input = "SELECT price::INTEGER FROM products";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_cast_with_params() {
        let input = "SELECT CAST(name AS VARCHAR(255)) FROM users";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_cast_decimal() {
        let input = "SELECT CAST(amount AS DECIMAL(10, 2)) FROM transactions";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_subquery_in_select() {
        let input = "SELECT (SELECT COUNT(*) FROM orders WHERE user_id = users.id) AS order_count FROM users";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_subquery_in_from() {
        let input = "SELECT * FROM (SELECT user_id, COUNT(*) AS cnt FROM orders GROUP BY user_id)";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_between() {
        let input = "SELECT * FROM products WHERE price BETWEEN 10 AND 100";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_between_with_expressions() {
        let input = "SELECT * FROM events WHERE created_at BETWEEN start_date AND end_date";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_in_values() {
        let input = "SELECT * FROM users WHERE status IN ('active', 'pending')";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_in_numbers() {
        let input = "SELECT * FROM products WHERE category_id IN (1, 2, 3, 5, 8)";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_in_subquery() {
        let input = "SELECT * FROM users WHERE id IN (SELECT user_id FROM orders WHERE total > 100)";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_exists() {
        let input = "SELECT * FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = users.id)";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_complex_nested_expressions() {
        let input = "SELECT CASE WHEN price::DECIMAL > 100 THEN 'expensive' ELSE 'cheap' END FROM products WHERE category_id IN (1, 2, 3)";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

    #[test]
    fn test_unary_minus() {
        let input = "SELECT -1 FROM users";
        let parse = parse(input);
        if !parse.errors.is_empty() {
            eprintln!("Errors: {:?}", parse.errors);
        }
        assert_eq!(parse.errors.len(), 0);
    }

}
